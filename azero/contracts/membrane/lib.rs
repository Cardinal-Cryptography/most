#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::membrane::{MembraneError, MembraneRef};

#[ink::contract]
pub mod membrane {
    use ink::{
        env::{
            call::{build_call, ExecutionInput},
            set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec, vec::Vec},
        storage::Mapping,
    };
    use psp22::{PSP22Error, PSP22};
    use psp22_traits::Mintable;
    use scale::{Decode, Encode};
    use shared::{concat_u8_arrays, keccak256, Keccak256HashOutput as HashedRequest, Selector};

    #[ink(event)]
    #[derive(Debug)]
    pub struct CrosschainTransferRequest {
        dest_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RequestProcessed {
        request_hash: HashedRequest,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RequestSigned {
        signer: AccountId,
        request_hash: HashedRequest,
    }

    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Request {
        signature_count: u128,
    }

    #[ink(storage)]
    pub struct Membrane {
        owner: AccountId,
        request_nonce: u128,
        signature_threshold: u128,
        pending_requests: Mapping<HashedRequest, Request>,
        signatures: Mapping<(HashedRequest, AccountId), ()>,
        processed_requests: Mapping<HashedRequest, ()>,
        guardians: Mapping<AccountId, ()>,
        supported_pairs: Mapping<[u8; 32], [u8; 32]>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        NotGuardian(AccountId),
        HashDoesNotMatchData,
        InvalidThreshold,
        PSP22(PSP22Error),
        RequestAlreadyProcessed,
        UnsupportedPair,
        InkEnvError(String),
        NotOwner(AccountId),
        RequestAlreadySigned,
        Arithmetic,
    }

    impl From<InkEnvError> for MembraneError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    impl From<PSP22Error> for MembraneError {
        fn from(inner: PSP22Error) -> Self {
            MembraneError::PSP22(inner)
        }
    }

    impl Membrane {
        #[ink(constructor)]
        pub fn new(
            guardians: Vec<AccountId>,
            signature_threshold: u128,
        ) -> Result<Self, MembraneError> {
            if signature_threshold == 0 || (guardians.len() as u128) < signature_threshold {
                return Err(MembraneError::InvalidThreshold);
            }

            let mut guardians_set = Mapping::new();
            guardians.into_iter().for_each(|account| {
                guardians_set.insert(account, &());
            });

            Ok(Self {
                owner: Self::env().caller(),
                request_nonce: 0,
                signature_threshold,
                pending_requests: Mapping::new(),
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                guardians: guardians_set,
                supported_pairs: Mapping::new(),
            })
        }

        /// Sets a new owner account
        ///
        /// Can only be called by contracts owner
        #[ink(message)]
        pub fn set_owner(&mut self, new_owner: AccountId) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.owner = new_owner;
            Ok(())
        }

        /// Adds a guardian account to the whitelist
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn add_guardian(&mut self, account: AccountId) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.guardians.insert(account, &());
            Ok(())
        }

        /// Removes a guardian account from the whitelist
        ///
        /// Can only be called by the contracts owner        
        #[ink(message)]
        pub fn remove_guardian(&mut self, account: AccountId) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.guardians.remove(account);
            Ok(())
        }

        /// Adds a supported pair for bridging
        ///
        /// Can only be called by the contracts owner                
        #[ink(message)]
        pub fn add_pair(&mut self, from: [u8; 32], to: [u8; 32]) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.supported_pairs.insert(from, &to);
            Ok(())
        }

        /// Removes a supported pair from bridging
        ///
        /// Can only be called by the contracts owner                        
        #[ink(message)]
        pub fn remove_pair(&mut self, from: [u8; 32]) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.supported_pairs.remove(from);
            Ok(())
        }

        /// Upgrades contract code
        #[ink(message)]
        pub fn set_code(
            &mut self,
            code_hash: [u8; 32],
            callback: Option<Selector>,
        ) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            set_code_hash(&code_hash)?;

            // Optionally call a callback function in the new contract that performs the storage data migration.
            // By convention this function should be called `migrate`, it should take no arguments
            // and be call-able only by `this` contract's instance address.
            // To ensure the latter the `migrate` in the updated contract can e.g. check if it has an Admin role on self.
            //
            // `delegatecall` ensures that the target contract is called within the caller contracts context.
            if let Some(selector) = callback {
                build_call::<DefaultEnvironment>()
                    .delegate(Hash::from(code_hash))
                    .exec_input(ExecutionInput::new(ink::env::call::Selector::new(selector)))
                    .returns::<Result<(), MembraneError>>()
                    .invoke()?;
            }

            Ok(())
        }

        /// Invoke this tx to initiate funds transfer to the destination chain.
        #[ink(message)]
        pub fn send_request(
            &mut self,
            src_token_address: [u8; 32],
            amount: Balance,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MembraneError> {
            let sender = self.env().caller();

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MembraneError::UnsupportedPair)?;

            self.transfer_from_tx(
                src_token_address.into(),
                sender,
                self.env().account_id(),
                amount,
            )?;

            self.env().emit_event(CrosschainTransferRequest {
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce: self.request_nonce,
            });

            self.request_nonce = self
                .request_nonce
                .checked_add(1)
                .ok_or(MembraneError::Arithmetic)?;

            Ok(())
        }

        /// Aggregates request votes cast by guardians and mints/burns tokens
        #[ink(message)]
        pub fn receive_request(
            &mut self,
            request_hash: HashedRequest,
            dest_token_address: [u8; 32],
            amount: u128,
            dest_receiver_address: [u8; 32],
            request_nonce: u128,
        ) -> Result<(), MembraneError> {
            let caller = self.env().caller();

            if !self.is_guardian(caller) {
                return Err(MembraneError::NotGuardian(caller));
            }

            if self.processed_requests.contains(request_hash) {
                return Err(MembraneError::RequestAlreadyProcessed);
            }

            let bytes = concat_u8_arrays(vec![
                &dest_token_address,
                &amount.to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.to_le_bytes(),
            ]);

            let hash = keccak256(&bytes);

            if !request_hash.eq(&hash) {
                return Err(MembraneError::HashDoesNotMatchData);
            }

            if self.signatures.contains((request_hash, caller)) {
                return Err(MembraneError::RequestAlreadySigned);
            }

            match self.pending_requests.get(request_hash) {
                None => {
                    self.pending_requests
                        .insert(request_hash, &Request { signature_count: 1 });

                    self.signatures.insert((request_hash, caller), &());

                    self.env().emit_event(RequestSigned {
                        signer: caller,
                        request_hash,
                    });
                }
                Some(mut request) => {
                    self.signatures.insert((request_hash, caller), &());

                    request.signature_count += 1;

                    self.env().emit_event(RequestSigned {
                        signer: caller,
                        request_hash,
                    });

                    if request.signature_count >= self.signature_threshold {
                        self.processed_requests.insert(request_hash, &());
                        self.signatures.remove((request_hash, caller));
                        self.pending_requests.remove(request_hash);

                        self.mint_to(
                            dest_token_address.into(),
                            dest_receiver_address.into(),
                            amount,
                        )?;
                    }

                    self.pending_requests.insert(request_hash, &request);

                    self.env().emit_event(RequestProcessed { request_hash });
                }
            }

            Ok(())
        }

        /// Returns a boolean value indicating whether given account is a guardian
        #[ink(message)]
        pub fn is_guardian(&self, account: AccountId) -> bool {
            self.guardians.contains(account)
        }

        fn ensure_owner(&mut self) -> Result<(), MembraneError> {
            let caller = self.env().caller();
            match caller.eq(&self.owner) {
                true => Ok(()),
                false => Err(MembraneError::NotOwner(caller)),
            }
        }

        /// Transfers a given amount of a PSP22 token on behalf of a specified account to another account
        ///
        /// Will revert if not enough allowance was given to the caller prior to executing this tx
        fn transfer_from_tx(
            &self,
            token: AccountId,
            from: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(PSP22) = token.into();
            psp22.transfer_from(from, to, amount, vec![])
        }

        /// Mints the specified amount of token to the designated account
        ///
        /// Membrane contract needs to have a Minter role on the token contract
        fn mint_to(
            &self,
            token: AccountId,
            to: AccountId,
            amount: Balance,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Mintable) = token.into();
            psp22.mint(to, amount)
        }
    }

    #[cfg(test)]
    mod tests {
        use ink::env::{
            test::{default_accounts, set_caller},
            DefaultEnvironment, Environment,
        };

        use super::*;

        const THRESHOLD: u128 = 3;
        type DefEnv = DefaultEnvironment;
        type AccountId = <DefEnv as Environment>::AccountId;

        fn guardian_accounts() -> Vec<AccountId> {
            let accounts = default_accounts::<DefEnv>();
            vec![
                accounts.bob,
                accounts.charlie,
                accounts.django,
                accounts.eve,
                accounts.frank,
            ]
        }

        #[ink::test]
        fn new_fails_on_zero_threshold() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            assert_eq!(
                Membrane::new(guardian_accounts(), 0)
                    .expect_err("Threshold is zero, instantiation should fail."),
                MembraneError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_fails_on_threshold_large_than_guardians() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            assert_eq!(
                Membrane::new(guardian_accounts(), (guardian_accounts().len() + 1) as u128)
                    .expect_err("Threshold is larger than guardians, instantiation should fail."),
                MembraneError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_sets_caller_as_owner() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            let mut membrane =
                Membrane::new(guardian_accounts(), THRESHOLD).expect("Threshold is valid.");

            assert_eq!(membrane.ensure_owner(), Ok(()));
            set_caller::<DefEnv>(guardian_accounts()[0]);
            assert_eq!(
                membrane.ensure_owner(),
                Err(MembraneError::NotOwner(guardian_accounts()[0]))
            );
        }

        #[ink::test]
        fn new_sets_correct_guardians() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let membrane =
                Membrane::new(guardian_accounts(), THRESHOLD).expect("Threshold is valid.");

            for account in guardian_accounts() {
                assert!(membrane.is_guardian(account));
            }
            assert!(!membrane.is_guardian(accounts.alice));
        }

        #[ink::test]
        fn set_owner_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut membrane =
                Membrane::new(guardian_accounts(), THRESHOLD).expect("Threshold is valid.");
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(
                membrane.ensure_owner(),
                Err(MembraneError::NotOwner(accounts.bob))
            );
            set_caller::<DefEnv>(accounts.alice);
            assert_eq!(membrane.ensure_owner(), Ok(()));
            assert_eq!(membrane.set_owner(accounts.bob), Ok(()));
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(membrane.ensure_owner(), Ok(()));
        }

        #[ink::test]
        fn add_guardian_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut membrane =
                Membrane::new(guardian_accounts(), THRESHOLD).expect("Threshold is valid.");

            assert!(!membrane.is_guardian(accounts.alice));
            assert_eq!(membrane.add_guardian(accounts.alice), Ok(()));
            assert!(membrane.is_guardian(accounts.alice));
        }

        #[ink::test]
        fn remove_guardian_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut membrane =
                Membrane::new(guardian_accounts(), THRESHOLD).expect("Threshold is valid.");

            assert!(membrane.is_guardian(accounts.bob));
            assert_eq!(membrane.remove_guardian(accounts.bob), Ok(()));
            assert!(!membrane.is_guardian(accounts.bob));
        }
    }
}
