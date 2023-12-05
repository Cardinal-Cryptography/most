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
    use psp22::PSP22Error;
    use psp22_traits::{Burnable, Mintable};
    use scale::{Decode, Encode};
    use shared::{concat_u8_arrays, keccak256, Keccak256HashOutput as HashedRequest, Selector};

    const DIX_MILLE: u128 = 10000;
    const NATIVE_TOKEN_ID: [u8; 32] = [0x0; 32];
    const WETH_TOKEN_ID: [u8; 32] = [0x1; 32];
    const USDT_TOKEN_ID: [u8; 32] = [0x2; 32];

    type CommitteeId = u128;

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct CrosschainTransferRequest {
        #[ink(topic)]
        pub committee_id: CommitteeId,
        #[ink(topic)]
        pub dest_token_address: [u8; 32],
        pub amount: u128,
        #[ink(topic)]
        pub dest_receiver_address: [u8; 32],
        pub request_nonce: u128,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct RequestProcessed {
        #[ink(topic)]
        pub request_hash: HashedRequest,
        pub dest_token_address: [u8; 32],
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct RequestSigned {
        #[ink(topic)]
        pub signer: AccountId,
        #[ink(topic)]
        pub request_hash: HashedRequest,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct SignedProcessedRequest {
        #[ink(topic)]
        pub request_hash: HashedRequest,
        #[ink(topic)]
        pub signer: AccountId,
    }

    #[derive(Default, Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
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
        /// requests that are still collecting signatures
        pending_requests: Mapping<HashedRequest, Request>,
        /// signatures per cross chain transfer request
        signatures: Mapping<(HashedRequest, AccountId), ()>,
        /// signed & executed requests, a replay protection
        processed_requests: Mapping<HashedRequest, ()>,
        /// set of guardian accounts that can sign requests
        committees: Mapping<(CommitteeId, AccountId), ()>,
        /// accounting helper
        committee_id: CommitteeId,
        /// accounting helper
        committee_sizes: Mapping<CommitteeId, u128>,
        /// number of signatures required to reach a quorum and costsxecute a transfer
        signature_thresholds: Mapping<CommitteeId, u128>,
        /// minimal value of tokens that can be transferred across the bridge
        minimum_transfer_amount_usd: u128,
        /// per mille of the succesfully transferred amount that is distributed among the guardians that have signed the crosschain transfer request
        commission_per_dix_mille: u128,
        /// a fixed subsidy transferred along with the bridged tokens to the destination account on aleph zero to bootstrap
        pocket_money: u128,
        /// source - destination token pairs that can be transferred across the bridge
        supported_pairs: Mapping<[u8; 32], [u8; 32]>,
        /// rewards collected by the commitee for relaying cross-chain transfer requests
        #[allow(clippy::type_complexity)]
        collected_committee_rewards: Mapping<(CommitteeId, [u8; 32]), u128>,
        /// rewards collected by the individual commitee members for relaying cross-chain transfer requests
        #[allow(clippy::type_complexity)]
        paid_out_member_rewards: Mapping<(AccountId, CommitteeId, [u8; 32]), u128>,
        /// How much gas does a single confirmation of a cross-chain transfer request use on the destination chain on average.
        /// This value is calculated by summing the total gas usage of *all* the transactions it takes to relay a single request and dividing it by the current committee size and multiplying by 1.2
        relay_gas_usage: u128,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        Constructor,
        InvalidThreshold,
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
            commission_per_dix_mille: u128,
            pocket_money: Balance,
            minimum_transfer_amount_usd: u128,
            relay_gas_usage: u128,
        ) -> Result<Self, MembraneError> {
            if commission_per_dix_mille.gt(&DIX_MILLE) {
                return Err(MembraneError::Constructor);
            }

            if signature_threshold == 0 || committee.len().lt(&(signature_threshold as usize)) {
                return Err(MembraneError::InvalidThreshold);
            }

            let committee_id = 0;

            let mut committees = Mapping::new();
            committee.clone().into_iter().for_each(|account| {
                committees.insert((committee_id, account), &());
            });

            let mut committee_sizes = Mapping::new();
            committee_sizes.insert(committee_id, &(committee.len() as u128));

            let mut signature_thresholds = Mapping::new();
            signature_thresholds.insert(committee_id, &signature_threshold);

            Ok(Self {
                owner: Self::env().caller(),
                request_nonce: 0,
                signature_thresholds,
                pending_requests: Mapping::new(),
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                committees,
                committee_id,
                committee_sizes,
                supported_pairs: Mapping::new(),
                collected_committee_rewards: Mapping::new(),
                paid_out_member_rewards: Mapping::new(),
                minimum_transfer_amount_usd,
                pocket_money,
                commission_per_dix_mille,
                relay_gas_usage,
            })
        }

        // --- business logic

        /// Invoke this tx to initiate funds transfer to the destination chain.
        ///
        /// Upon checking basic conditions the contract will burn the `amount` number of `src_token_address` tokens from the caller
        /// and emit an event which is to be picked up & acted on up by the bridge guardians.
        #[ink(message, payable)]
        pub fn send_request(
            &mut self,
            src_token_address: [u8; 32],
            amount: u128,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MembraneError> {
            if self
                .query_price(amount, src_token_address, USDT_TOKEN_ID)?
                .lt(&self.minimum_transfer_amount_usd)
            {
                return Err(MembraneError::AmountBelowMinimum);
            }

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MembraneError::UnsupportedPair)?;

            let current_base_fee = self.base_fee()?;
            let base_fee = self.env().transferred_value();

            if base_fee.lt(&current_base_fee) {
                return Err(MembraneError::BaseFeeTooLow);
            }

            let sender = self.env().caller();

            // burn the psp22 tokens
            self.burn_from(src_token_address.into(), sender, amount)?;

            // NOTE: this allows the committee members to take a payout for requests that are not neccessarily finished
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

            // NOTE: this allows the committee members to take a payout for requests that are not neccessarily finished
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
                committee_id: self.committee_id,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce: self.request_nonce,
            });

            self.request_nonce = self
                .request_nonce
                .checked_add(1)
                .ok_or(MembraneError::Arithmetic)?;

            // return surplus if any
            if let Some(surplus) = base_fee.checked_sub(current_base_fee) {
                self.env().transfer(sender, surplus)?;
            };

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

            self.only_current_committee_member(caller)?;

            // Don't revert if the request has already been processed as
            // such a call can be made during regular guardian operation.
            if self.processed_requests.contains(request_hash) {
                self.env().emit_event(SignedProcessedRequest {
                    request_hash,
                    signer: caller,
                });
                return Ok(());
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

            let mut request = self.pending_requests.get(request_hash).unwrap_or_default(); //  {

            // record vote
            request.signature_count += 1;
            self.signatures.insert((request_hash, caller), &());

            self.env().emit_event(RequestSigned {
                signer: caller,
                request_hash,
            });

            let signature_threshold = self
                .signature_thresholds
                .get(self.committee_id)
                .ok_or(MembraneError::InvalidThreshold)?;

            if request.signature_count >= signature_threshold {
                let commission = amount
                    .checked_mul(self.commission_per_dix_mille)
                    .ok_or(MembraneError::Arithmetic)?
                    .checked_div(DIX_MILLE)
                    .ok_or(MembraneError::Arithmetic)?;

                let updated_commission_total = self
                    .get_collected_committee_rewards(self.committee_id, dest_token_address)
                    .checked_add(commission)
                    .ok_or(MembraneError::Arithmetic)?;

                self.mint_to(
                    dest_token_address.into(),
                    dest_receiver_address.into(),
                    amount
                        .checked_sub(commission)
                        .ok_or(MembraneError::Arithmetic)?,
                )?;

                // bootstrap account with pocket money
                // NOTE: we don't revert on a failure!
                _ = self
                    .env()
                    .transfer(dest_receiver_address.into(), self.pocket_money);

                self.collected_committee_rewards.insert(
                    (self.committee_id, dest_token_address),
                    &updated_commission_total,
                );

                // mark it as processed
                self.processed_requests.insert(request_hash, &());
                self.pending_requests.remove(request_hash);

                self.env().emit_event(RequestProcessed {
                    request_hash,
                    dest_token_address,
                });
            } else {
                self.pending_requests.insert(request_hash, &request);
            }

            Ok(())
        }

        /// Request payout of rewards for signing & relaying cross-chain transfers.
        ///
        /// Can be called by anyone on behalf of the committee member.
        #[ink(message)]
        pub fn payout_rewards(
            &mut self,
            committee_id: CommitteeId,
            member_id: AccountId,
            token_id: [u8; 32],
        ) -> Result<(), MembraneError> {
            let paid_out_rewards =
                self.get_paid_out_member_rewards(committee_id, member_id, token_id);

            let outstanding_rewards =
                self.get_outstanding_member_rewards(committee_id, member_id, token_id)?;

            if outstanding_rewards.gt(&0) {
                match token_id {
                    NATIVE_TOKEN_ID => {
                        self.env().transfer(member_id, outstanding_rewards)?;
                    }
                    _ => {
                        self.mint_to(token_id.into(), member_id, outstanding_rewards)?;
                    }
                }

                self.paid_out_member_rewards.insert(
                    (member_id, committee_id, token_id),
                    &paid_out_rewards
                        .checked_add(outstanding_rewards)
                        .ok_or(MembraneError::Arithmetic)?,
                );
            }

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

        // ---  getter txs

        /// Query pocket money
        ///
        /// An amount of the native token that is tranferred with every request
        #[ink(message)]
        pub fn get_pocket_money(&self) -> Balance {
            self.pocket_money
        }

        /// Query total rewards for this committee
        ///
        /// Denominated in token with the `token_id` address
        /// Uses [0u8;32] to identify the native token
        #[ink(message)]
        pub fn get_collected_committee_rewards(
            &self,
            committee_id: CommitteeId,
            token_id: [u8; 32],
        ) -> u128 {
            self.collected_committee_rewards
                .get((committee_id, token_id))
                .unwrap_or_default()
        }

        /// Query already paid out committee member rewards
        ///
        /// Denominated in token with the `token_id` address
        /// Uses [0u8;32] to identify the native token
        #[ink(message)]
        pub fn get_paid_out_member_rewards(
            &self,
            committee_id: CommitteeId,
            member_id: AccountId,
            token_id: [u8; 32],
        ) -> u128 {
            self.paid_out_member_rewards
                .get((member_id, committee_id, token_id))
                .unwrap_or_default()
        }

        /// Query outstanding committee member rewards
        ///
        /// The amount that can still be requested.
        /// Denominated in token with the `token_id` address
        /// Uses [0u8;32] to identify the native token
        /// Returns an error (reverts) if the `member_id` account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn get_outstanding_member_rewards(
            &self,
            committee_id: CommitteeId,
            member_id: AccountId,
            token_id: [u8; 32],
        ) -> Result<u128, MembraneError> {
            let total_amount = self
                .get_collected_committee_rewards(committee_id, token_id)
                .checked_div(
                    self.committee_sizes
                        .get(committee_id)
                        .ok_or(MembraneError::NotInCommittee)?,
                )
                .ok_or(MembraneError::Arithmetic)?;

            let collected_amount =
                self.get_paid_out_member_rewards(committee_id, member_id, token_id);

            Ok(total_amount.saturating_sub(collected_amount))
        }

        /// Queries a gas price oracle and returns the current base_fee charged per cross chain transfer denominated in AZERO
        #[ink(message)]
        pub fn base_fee(&self) -> Result<Balance, MembraneError> {
            // TODO: implement
            // return a current gas price in WEI
            let do_query_gas_fee = || 39106342561;

            let amount = self
                .relay_gas_usage
                .checked_mul(do_query_gas_fee())
                .ok_or(MembraneError::Arithmetic)?;

            self.query_price(amount, WETH_TOKEN_ID, NATIVE_TOKEN_ID)
        }

        /// Returns current active committee id
        #[ink(message)]
        pub fn current_committee_id(&self) -> u128 {
            self.committee_id
        }

        /// Returns an error (reverts) if account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn is_in_committee(&self, committee_id: CommitteeId, account: AccountId) -> bool {
            self.committees.contains((committee_id, account))
        }

        /// Returns an error (reverts) if account is not in the currently active committee
        #[ink(message)]
        pub fn only_current_committee_member(
            &self,
            account: AccountId,
        ) -> Result<(), MembraneError> {
            match self.is_in_committee(self.committee_id, account) {
                true => Ok(()),
                false => Err(MembraneError::NotInCommittee),
            }
        }

        #[ink(message)]
        pub fn get_commission_per_dix_mille(&self) -> u128 {
            self.commission_per_dix_mille
        }

        // ---  setter txs

        /// Removes a supported pair from bridging
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn remove_pair(&mut self, from: [u8; 32]) -> Result<(), MembraneError> {
            self.ensure_owner()?;
            self.supported_pairs.remove(from);
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

        /// Change the committee and increase committe id
        /// Can only be called by the contracts owner
        ///
        /// Changing the entire set is the ONLY way of upgrading the committee
        #[ink(message)]
        pub fn set_committee(
            &mut self,
            committee: Vec<AccountId>,
            signature_threshold: u128,
        ) -> Result<(), MembraneError> {
            self.ensure_owner()?;

            if signature_threshold == 0 || committee.len().lt(&(signature_threshold as usize)) {
                return Err(MembraneError::InvalidThreshold);
            }

            let committee_id = self.committee_id + 1;
            let mut committee_set = Mapping::new();
            committee.into_iter().for_each(|account| {
                committee_set.insert((committee_id, account), &());
            });

            self.committees = committee_set;
            self.committee_id = committee_id;

            self.signature_thresholds
                .insert(committee_id, &signature_threshold);

            Ok(())
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

        // ---  helper functions

        /// Queries a price oracle and returns the price of an `amount` number of the `of` tokens denominated in the `in_token`
        ///
        /// TODO: this is a mocked method pending an implementation
        #[ink(message)]
        pub fn query_price(
            &self,
            amount_of: u128,
            of_token_address: [u8; 32],
            in_token_address: [u8; 32],
        ) -> Result<u128, MembraneError> {
            if in_token_address == USDT_TOKEN_ID {
                return Ok(amount_of * 2);
            }

            if of_token_address == USDT_TOKEN_ID {
                return Ok(amount_of / 2);
            }

            Ok(amount_of)
        }

        fn ensure_owner(&mut self) -> Result<(), MembraneError> {
            let caller = self.env().caller();
            match caller.eq(&self.owner) {
                true => Ok(()),
                false => Err(MembraneError::NotOwner(caller)),
            }
        }

        /// Mints the specified amount of token to the designated account
        ///
        /// Membrane contract needs to have a Minter role on the token contract
        fn mint_to(&self, token: AccountId, to: AccountId, amount: u128) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Mintable) = token.into();
            psp22.mint(to, amount)
        }

        /// Burn the specified amount of token from the designated account
        ///
        /// Membrane contract needs to have a Burner role on the token contract
        fn burn_from(
            &self,
            token: AccountId,
            from: AccountId,
            amount: u128,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Burnable) = token.into();
            psp22.burn(from, amount)
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
        const COMMISSION_PER_DIX_MILLE: u128 = 30;
        const POCKET_MONEY: Balance = 1000000000000;
        const MINIMUM_TRANSFER_AMOUNT_USD: u128 = 50;
        const RELAY_GAS_USAGE: u128 = 50000;

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
                Membrane::new(
                    guardian_accounts(),
                    0,
                    COMMISSION_PER_DIX_MILLE,
                    POCKET_MONEY,
                    MINIMUM_TRANSFER_AMOUNT_USD,
                    RELAY_GAS_USAGE
                )
                .expect_err("Threshold is zero, instantiation should fail."),
                MembraneError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_fails_on_threshold_large_than_guardians() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            assert_eq!(
                Membrane::new(
                    guardian_accounts(),
                    (guardian_accounts().len() + 1) as u128,
                    COMMISSION_PER_DIX_MILLE,
                    POCKET_MONEY,
                    MINIMUM_TRANSFER_AMOUNT_USD,
                    RELAY_GAS_USAGE
                )
                .expect_err("Threshold is larger than guardians, instantiation should fail."),
                MembraneError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_sets_caller_as_owner() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            let mut membrane = Membrane::new(
                guardian_accounts(),
                THRESHOLD,
                COMMISSION_PER_DIX_MILLE,
                POCKET_MONEY,
                MINIMUM_TRANSFER_AMOUNT_USD,
                RELAY_GAS_USAGE,
            )
            .expect("Threshold is valid.");

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
            let membrane = Membrane::new(
                guardian_accounts(),
                THRESHOLD,
                COMMISSION_PER_DIX_MILLE,
                POCKET_MONEY,
                MINIMUM_TRANSFER_AMOUNT_USD,
                RELAY_GAS_USAGE,
            )
            .expect("Threshold is valid.");

            for account in guardian_accounts() {
                assert!(membrane.is_in_committee(membrane.current_committee_id(), account));
            }
            assert!(!membrane.is_in_committee(membrane.current_committee_id(), accounts.alice));
        }

        #[ink::test]
        fn set_owner_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut membrane = Membrane::new(
                guardian_accounts(),
                THRESHOLD,
                COMMISSION_PER_DIX_MILLE,
                POCKET_MONEY,
                MINIMUM_TRANSFER_AMOUNT_USD,
                RELAY_GAS_USAGE,
            )
            .expect("Threshold is valid.");
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
            let mut membrane = Membrane::new(
                guardian_accounts(),
                THRESHOLD,
                COMMISSION_PER_DIX_MILLE,
                POCKET_MONEY,
                MINIMUM_TRANSFER_AMOUNT_USD,
                RELAY_GAS_USAGE,
            )
            .expect("Threshold is valid.");

            assert!(!membrane.is_in_committee(membrane.current_committee_id(), accounts.alice));
            assert_eq!(membrane.set_committee(vec![accounts.alice], 1), Ok(()));
            assert!(membrane.is_in_committee(membrane.current_committee_id(), accounts.alice));
        }

        #[ink::test]
        fn remove_guardian_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut membrane = Membrane::new(
                guardian_accounts(),
                THRESHOLD,
                COMMISSION_PER_DIX_MILLE,
                POCKET_MONEY,
                MINIMUM_TRANSFER_AMOUNT_USD,
                RELAY_GAS_USAGE,
            )
            .expect("Threshold is valid.");

            assert!(membrane.is_in_committee(membrane.current_committee_id(), accounts.bob));
            assert_eq!(membrane.set_committee(vec![accounts.alice], 1), Ok(()));
            assert!(!membrane.is_in_committee(membrane.current_committee_id(), accounts.bob));
        }
    }
}
