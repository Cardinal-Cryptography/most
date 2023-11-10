#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod membrane {

    use ink::{
        env::{
            call::{build_call, ExecutionInput},
            set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{collections::BTreeMap, format, string::String, vec, vec::Vec},
        storage::Mapping,
    };
    use psp22::{PSP22Error, PSP22};
    use psp22_traits::Mintable;
    use scale::{Decode, Encode};
    use shared::{concat_u8_arrays, keccak256, Keccak256HashOutput as HashedRequest, Selector};

    const MILLE: u128 = 1000;

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
        dest_token_address: [u8; 32],
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
        /// an account that can perform a subset of actions
        owner: AccountId,
        /// nonce for outgoing cross-chain transfer requests
        request_nonce: u128,
        /// number of signatures required to reach a quorum and execute a transfer
        signature_threshold: u128,
        /// requests that are still collecting signatures
        pending_requests: Mapping<HashedRequest, Request>,
        /// signatures per cross chain transfer request
        signatures: Mapping<(HashedRequest, AccountId), ()>,
        /// signed & executed requests, a replay protection
        processed_requests: Mapping<HashedRequest, ()>,
        /// set of guardian accounts that can sign requests
        guardians: Mapping<AccountId, ()>,
        /// accounting helper data structure
        guardians_count: u128,
        /// minimal amount of tokens that can be transferred across the bridge
        minimum_transfer_amount: Balance,
        /// base fee paid in the source chains native token that is distributed among the guardians, set to track the gas costs of signing the relay transactions on the destination chain
        base_fee: Balance,
        /// per mille of the succesfully transferred amount that is distributed among the guardians that have signed the crosschain transfer request
        commission_per_mille: u128,
        /// a fixed subsidy transferred along with the bridged tokens to the destination account on aleph zero to bootstrap
        pocket_money: Balance,
        /// rewards collected by the commitee members for relaying cross-chain transfer requests, denominated in the bridged token representation on the destination chain
        #[allow(clippy::type_complexity)]
        commissions: Mapping<HashedRequest, ([u8; 32], Balance)>,
        /// remuneration collected by the commitee members to cover the gas fees for signing the requests on the destination chain. Denominated in the source chain native currency
        base_fees: Mapping<HashedRequest, Balance>,
        /// accounting data structure
        collected_rewards: Mapping<(HashedRequest, AccountId), ()>,
        /// source - destination token pairs that can be transferred across the bridge
        supported_pairs: Mapping<[u8; 32], [u8; 32]>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        Constructor,
        NotGuardian,
        HashDoesNotMatchData,
        RewardAlreadyCollected,
        PSP22(PSP22Error),
        RequestNotProcessed,
        RequestAlreadyProcessed,
        UnsupportedPair,
        AmountBelowMinimum,
        InkEnvError(String),
        NotOwner(AccountId),
        RequestAlreadySigned,
        BaseFeeTooLow,
        Arithmetic,
        NoRewards,
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
            commission_per_mille: u128,
            base_fee: Balance,
            pocket_money: Balance,
            minimum_transfer_amount: Balance,
        ) -> Result<Self, MembraneError> {
            if commission_per_mille.gt(&1000) {
                return Err(MembraneError::Constructor);
            }

            if guardians.len().lt(&(signature_threshold as usize)) {
                return Err(MembraneError::Constructor);
            }

            let guardians_count = guardians.len() as u128;

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
                guardians_count,
                supported_pairs: Mapping::new(),
                commissions: Mapping::new(),
                base_fees: Mapping::new(),
                collected_rewards: Mapping::new(),
                minimum_transfer_amount,
                pocket_money,
                base_fee,
                commission_per_mille,
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

            if !self.guardians.contains(account) {
                self.guardians.insert(account, &());
                self.guardians_count += 1
            }

            Ok(())
        }

        /// Removes a guardian account from the whitelist
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn remove_guardian(&mut self, account: AccountId) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.guardians.remove(account);
            self.guardians_count -= 1;
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
        #[ink(message, payable)]
        pub fn send_request(
            &mut self,
            src_token_address: [u8; 32],
            amount: Balance,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MembraneError> {
            if amount.lt(&self.minimum_transfer_amount) {
                return Err(MembraneError::AmountBelowMinimum);
            }

            let sender = self.env().caller();
            self.transfer_from_tx(
                src_token_address.into(),
                sender,
                self.env().account_id(),
                amount,
            )?;

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MembraneError::UnsupportedPair)?;

            let base_fee = self.env().transferred_value();
            if base_fee.lt(&self.base_fee) {
                return Err(MembraneError::BaseFeeTooLow);
            }

            let bytes = concat_u8_arrays(vec![
                &dest_token_address,
                &amount.to_le_bytes(),
                &dest_receiver_address,
                &self.request_nonce.to_le_bytes(),
            ]);

            let hash = keccak256(&bytes);

            self.base_fees.insert(hash, &base_fee);

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
            self.is_guardian(caller)?;

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
                        self.mint_to(
                            dest_token_address.into(),
                            dest_receiver_address.into(),
                            amount,
                        )?;

                        // bootstrap account with pocket money
                        self.env()
                            .transfer(dest_receiver_address.into(), self.pocket_money)?;

                        // insert reward record for signing this transfer
                        let reward = amount
                            .checked_mul(self.commission_per_mille)
                            .ok_or(MembraneError::Arithmetic)?
                            .checked_div(MILLE)
                            .ok_or(MembraneError::Arithmetic)?;

                        self.commissions
                            .insert(request_hash, &(dest_token_address, reward));

                        // mark it as processed
                        self.processed_requests.insert(request_hash, &());

                        // clean up
                        self.signatures.remove((request_hash, caller));
                        self.pending_requests.remove(request_hash);

                        self.env().emit_event(RequestProcessed {
                            request_hash,
                            dest_token_address,
                        });
                    } else {
                        self.pending_requests.insert(request_hash, &request);
                    }
                }
            }

            Ok(())
        }

        /// Request payout of rewards for multiple cross-chain transer requests identified by their hashes.
        /// Requests need to be fully processed to continue.
        ///
        /// Can be called by anyone on behalf of the relayer.
        #[ink(message)]
        pub fn payout_rewards(
            &mut self,
            requests: Vec<HashedRequest>,
            to: AccountId,
        ) -> Result<(), MembraneError> {
            let mut base_fee_total = 0;
            let mut commission_total = BTreeMap::new();

            for request_hash in requests {
                if self.pending_requests.contains(request_hash) {
                    return Err(MembraneError::RequestNotProcessed);
                }

                if self.collected_rewards.contains((request_hash, to)) {
                    return Err(MembraneError::RewardAlreadyCollected);
                }

                // commission
                let (token, amount) = self
                    .commissions
                    .get(request_hash)
                    .ok_or(MembraneError::NoRewards)?;

                let commission = amount
                    .checked_div(self.guardians_count)
                    .ok_or(MembraneError::Arithmetic)?;

                let total_reward = commission_total.get(&token).unwrap_or(&0u128);

                let base_fee = self
                    .base_fees
                    .get(request_hash)
                    .ok_or(MembraneError::NoRewards)?
                    .checked_div(self.guardians_count)
                    .ok_or(MembraneError::Arithmetic)?;

                commission_total.insert(token, total_reward + commission);
                base_fee_total += base_fee;

                // mark rewards as collected
                self.collected_rewards.insert((request_hash, to), &());
            }

            for (token, amount) in commission_total {
                self.mint_to(token.into(), to, amount)?;
            }

            self.env().transfer(to, base_fee_total)?;

            Ok(())
        }

        /// Request payout of the reward for signing and relaying a single cross-chain transer request identified by it's hash.
        /// Request needs to be fully processed in order to continue.
        ///
        /// Can be called by anyone on behalf of the relayer.
        #[ink(message)]
        pub fn payout_reward(
            &mut self,
            request_hash: HashedRequest,
            to: AccountId,
        ) -> Result<(), MembraneError> {
            if self.pending_requests.contains(request_hash) {
                return Err(MembraneError::RequestNotProcessed);
            }

            if self.collected_rewards.contains((request_hash, to)) {
                return Err(MembraneError::RewardAlreadyCollected);
            }

            // base fee
            let base_fee = self
                .base_fees
                .get(request_hash)
                .ok_or(MembraneError::NoRewards)?
                .checked_div(self.guardians_count)
                .ok_or(MembraneError::Arithmetic)?;

            // commission
            let (token, amount) = self
                .commissions
                .get(request_hash)
                .ok_or(MembraneError::NoRewards)?;

            let commission = amount
                .checked_div(self.guardians_count)
                .ok_or(MembraneError::Arithmetic)?;

            self.mint_to(token.into(), to, commission)?;
            self.env().transfer(to, base_fee)?;

            // mark rewards as collected
            self.collected_rewards.insert((request_hash, to), &());

            Ok(())
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

        fn is_guardian(&self, account: AccountId) -> Result<(), MembraneError> {
            if self.guardians.contains(account) {
                Ok(())
            } else {
                Err(MembraneError::NotGuardian)
            }
        }
    }
}
