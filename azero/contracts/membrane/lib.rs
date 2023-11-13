#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod membrane {

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

    const MILLE: u128 = 1000;
    const NATIVE_TOKEN_ID: [u8; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    type CommitteeId = u128;

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
        committee: Mapping<(CommitteeId, AccountId), ()>,
        /// accounting helper
        committee_id: CommitteeId,
        /// accounting helper
        committee_size: Mapping<CommitteeId, u128>,
        /// minimal amount of tokens that can be transferred across the bridge
        minimum_transfer_amount: Balance,
        /// base fee paid in the source chains native token that is distributed among the guardians, set to track the gas costs of signing the relay transactions on the destination chain
        base_fee: Balance,
        /// per mille of the succesfully transferred amount that is distributed among the guardians that have signed the crosschain transfer request
        commission_per_mille: u128,
        /// a fixed subsidy transferred along with the bridged tokens to the destination account on aleph zero to bootstrap
        pocket_money: Balance,
        /// source - destination token pairs that can be transferred across the bridge
        supported_pairs: Mapping<[u8; 32], [u8; 32]>,
        /// rewards collected by the commitee for relaying cross-chain transfer requests                
        #[allow(clippy::type_complexity)]
        collected_committee_rewards: Mapping<(CommitteeId, [u8; 32]), Balance>,
        /// rewards collected by the individual commitee members for relaying cross-chain transfer requests        
        #[allow(clippy::type_complexity)]
        collected_member_rewards: Mapping<(AccountId, CommitteeId, [u8; 32]), Balance>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        Constructor,
        NotInCommittee,
        HashDoesNotMatchData,
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
        NoMoreRewards,
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
            committee: Vec<AccountId>,
            signature_threshold: u128,
            commission_per_mille: u128,
            base_fee: Balance,
            pocket_money: Balance,
            minimum_transfer_amount: Balance,
        ) -> Result<Self, MembraneError> {
            if commission_per_mille.gt(&1000) {
                return Err(MembraneError::Constructor);
            }

            if committee.len().lt(&(signature_threshold as usize)) {
                return Err(MembraneError::Constructor);
            }

            let committee_id = 0;

            let mut committee_set = Mapping::new();
            committee.clone().into_iter().for_each(|account| {
                committee_set.insert((committee_id, account), &());
            });

            let mut committee_size = Mapping::new();
            committee_size.insert(committee_id, &(committee.len() as u128));

            Ok(Self {
                owner: Self::env().caller(),
                request_nonce: 0,
                signature_threshold,
                pending_requests: Mapping::new(),
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                committee: committee_set,
                committee_id,
                committee_size,
                supported_pairs: Mapping::new(),
                collected_committee_rewards: Mapping::new(),
                collected_member_rewards: Mapping::new(),
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

        pub fn set_committee(&mut self, committee: Vec<AccountId>) -> Result<(), MembraneError> {
            self.ensure_owner()?;

            let committee_id = self.committee_id + 1;

            let mut committee_set = Mapping::new();
            committee.into_iter().for_each(|account| {
                committee_set.insert((committee_id, account), &());
            });

            self.committee = committee_set;
            self.committee_id = committee_id;

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

        // TODO : get base_fee (to be later changed to an Oracle call)
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

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MembraneError::UnsupportedPair)?;

            let base_fee = self.env().transferred_value();
            if base_fee.lt(&self.base_fee) {
                return Err(MembraneError::BaseFeeTooLow);
            }

            let sender = self.env().caller();

            self.transfer_from_tx(
                src_token_address.into(),
                sender,
                self.env().account_id(),
                amount,
            )?;

            // record base fee as collected
            // PROBLEM: this allows the committee members to take a payout for requests that are not neccessarily finished
            // by that time (no signature threshold reached yet).
            // We could be recording the base fee when the request collects quorum, but it could change in the meantime
            // which is potentially even worse
            let base_fee_total = self
                .collected_committee_rewards
                .get((self.committee_id, NATIVE_TOKEN_ID))
                .unwrap_or(0)
                .checked_add(base_fee)
                .ok_or(MembraneError::Arithmetic)?;

            self.collected_committee_rewards
                .insert((self.committee_id, NATIVE_TOKEN_ID), &base_fee_total);

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

                        let commission = amount
                            .checked_mul(self.commission_per_mille)
                            .ok_or(MembraneError::Arithmetic)?
                            .checked_div(MILLE)
                            .ok_or(MembraneError::Arithmetic)?;

                        let commission_total = self
                            .collected_committee_rewards
                            .get((self.committee_id, dest_token_address))
                            .unwrap_or(0)
                            .checked_add(commission)
                            .ok_or(MembraneError::Arithmetic)?;

                        self.collected_committee_rewards
                            .insert((self.committee_id, dest_token_address), &commission_total);

                        // insert reward record for signing this transfer
                        // let reward = amount
                        //     .checked_mul(self.commission_per_mille)
                        //     .ok_or(MembraneError::Arithmetic)?
                        //     .checked_div(MILLE)
                        //     .ok_or(MembraneError::Arithmetic)?;

                        // self.commissions
                        //     .insert(request_hash, &(dest_token_address, reward));

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

        /// Request payout of rewards for signing & relaying cross-chain transfers.
        ///
        /// Can be called by anyone on behalf of a committee member.
        #[ink(message)]
        pub fn payout_rewards(
            &mut self,
            committee_id: CommitteeId,
            member_id: AccountId,
            token_id: [u8; 32],
        ) -> Result<(), MembraneError> {
            let total_amount = self
                .collected_committee_rewards
                .get((committee_id, token_id))
                .ok_or(MembraneError::NoRewards)?
                .checked_div(
                    self.committee_size
                        .get(committee_id)
                        .ok_or(MembraneError::NotInCommittee)?,
                )
                .ok_or(MembraneError::Arithmetic)?;

            let collected_amount = self
                .collected_member_rewards
                .get((member_id, committee_id, token_id))
                .ok_or(MembraneError::NoRewards)?;

            if collected_amount >= total_amount {
                return Err(MembraneError::NoMoreRewards);
            }

            let amount = total_amount
                .checked_sub(collected_amount)
                .ok_or(MembraneError::Arithmetic)?;

            match token_id {
                NATIVE_TOKEN_ID => {
                    self.env().transfer(member_id, amount)?;
                }
                _ => {
                    self.mint_to(token_id.into(), member_id, amount)?;
                }
            }

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
            if self.committee.contains((self.committee_id, account)) {
                Ok(())
            } else {
                Err(MembraneError::NotInCommittee)
            }
        }
    }
}
