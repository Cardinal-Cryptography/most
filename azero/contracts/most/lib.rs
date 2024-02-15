#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::most::{MostError, MostRef};

#[ink::contract]
pub mod most {

    use gas_oracle_trait::EthGasPriceOracle;
    use ink::{
        codegen::TraitCallBuilder,
        contract_ref,
        env::{
            call::{build_call, ExecutionInput},
            set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec, vec::Vec},
        storage::{traits::ManualKey, Lazy, Mapping},
    };
    use psp22::PSP22Error;
    use psp22_traits::{Burnable, Mintable};
    use scale::{Decode, Encode};
    use shared::{concat_u8_arrays, keccak256, Keccak256HashOutput as HashedRequest, Selector};

    type CommitteeId = u128;

    const GAS_ORACLE_MAX_AGE: u64 = 24 * 60 * 60 * 1000; // 1 day
    const ORACLE_CALL_GAS_LIMIT: u64 = 2_000_000_000;
    const BASE_FEE_BUFFER_PERCENTAGE: u128 = 20;

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
        pub request_hash: HashedRequest,
        #[ink(topic)]
        pub dest_token_address: [u8; 32],
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct RequestSigned {
        pub request_hash: HashedRequest,
        #[ink(topic)]
        pub signer: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct HaltedStateChanged {
        pub previous_state: bool,
        pub new_state: bool,
        #[ink(topic)]
        pub caller: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct SignedProcessedRequest {
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

    #[derive(Debug)]
    #[ink::storage_item]
    pub struct Data {
        /// an account that can perform a subset of actions
        owner: AccountId,
        /// nonce for outgoing cross-chain transfer requests
        request_nonce: u128,
        /// accounting helper
        committee_id: CommitteeId,
        /// a fixed subsidy transferred along with the bridged tokens to the destination account on aleph zero to bootstrap
        pocket_money: u128,
        /// balance that can be paid out as pocket money
        pocket_money_balance: u128,
        /// How much gas does a single confirmation of a cross-chain transfer request use on the destination chain on average.
        /// This value is calculated by summing the total gas usage of *all* the transactions it takes to relay a single request and dividing it by the current committee size and multiplying by 1.2
        relay_gas_usage: u128,
        /// minimum fee that can be charged for a cross-chain transfer request
        min_fee: u128,
        /// maximum fee that can be charged for a cross-chain transfer request
        max_fee: u128,
        /// default fee that is charged for a cross-chain transfer request if the gas price oracle is not available/malfunctioning
        default_fee: u128,
        /// gas price oracle that is used to calculate the fee for a cross-chain transfer request
        gas_price_oracle: Option<AccountId>,
        /// Is the bridge in a halted state
        is_halted: bool,
    }

    #[ink(storage)]
    pub struct Most {
        data: Lazy<Data, ManualKey<0x44415441>>,
        /// requests that are still collecting signatures
        pending_requests: Mapping<HashedRequest, Request, ManualKey<0x50454E44>>,
        /// signatures per cross chain transfer request
        signatures: Mapping<(HashedRequest, AccountId), (), ManualKey<0x5349474E>>,
        /// signed & executed requests, a replay protection
        processed_requests: Mapping<HashedRequest, (), ManualKey<0x50524F43>>,
        /// set of guardian accounts that can sign requests
        committees: Mapping<(CommitteeId, AccountId), (), ManualKey<0x434F4D4D>>,
        /// accounting helper
        committee_sizes: Mapping<CommitteeId, u128, ManualKey<0x53495A45>>,
        /// number of signatures required to reach a quorum and costsxecute a transfer
        signature_thresholds: Mapping<CommitteeId, u128, ManualKey<0x54485245>>,
        /// source - destination token pairs that can be transferred across the bridge
        supported_pairs: Mapping<[u8; 32], [u8; 32], ManualKey<0x53555050>>,
        /// rewards collected by the commitee for relaying cross-chain transfer requests
        collected_committee_rewards: Mapping<CommitteeId, u128, ManualKey<0x434F4C4C>>,
        /// rewards collected by the individual commitee members for relaying cross-chain transfer requests
        paid_out_member_rewards: Mapping<(AccountId, CommitteeId), u128, ManualKey<0x50414944>>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MostError {
        Constructor,
        InvalidThreshold,
        NotInCommittee,
        HashDoesNotMatchData,
        PSP22(PSP22Error),
        RequestNotProcessed,
        RequestAlreadyProcessed,
        UnsupportedPair,
        InkEnvError(String),
        NotOwner(AccountId),
        RequestAlreadySigned,
        BaseFeeTooLow,
        Arithmetic,
        NoRewards,
        NoMoreRewards,
        CorruptedStorage,
        IsHalted,
    }

    impl From<InkEnvError> for MostError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    impl From<PSP22Error> for MostError {
        fn from(inner: PSP22Error) -> Self {
            MostError::PSP22(inner)
        }
    }

    impl Most {
        #[allow(clippy::too_many_arguments)]
        #[ink(constructor)]
        pub fn new(
            committee: Vec<AccountId>,
            signature_threshold: u128,
            pocket_money: Balance,
            relay_gas_usage: u128,
            min_fee: Balance,
            max_fee: Balance,
            default_fee: Balance,
            gas_price_oracle: Option<AccountId>,
        ) -> Result<Self, MostError> {
            if signature_threshold == 0 || committee.len().lt(&(signature_threshold as usize)) {
                return Err(MostError::InvalidThreshold);
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

            let mut data = Lazy::new();
            data.set(&Data {
                owner: Self::env().caller(),
                request_nonce: 0,
                committee_id,
                pocket_money,
                pocket_money_balance: 0,
                relay_gas_usage,
                min_fee,
                max_fee,
                default_fee,
                gas_price_oracle,
                is_halted: false,
            });

            Ok(Self {
                data,
                signature_thresholds,
                committees,
                committee_sizes,
                pending_requests: Mapping::new(),
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                supported_pairs: Mapping::new(),
                collected_committee_rewards: Mapping::new(),
                paid_out_member_rewards: Mapping::new(),
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
        ) -> Result<(), MostError> {
            self.check_halted()?;

            let mut data = self.data()?;

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MostError::UnsupportedPair)?;

            let current_base_fee = self.get_base_fee()?;
            let transferred_fee = self.env().transferred_value();

            if transferred_fee.lt(&current_base_fee) {
                return Err(MostError::BaseFeeTooLow);
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
                .get(data.committee_id)
                .unwrap_or(0)
                .checked_add(current_base_fee)
                .ok_or(MostError::Arithmetic)?;

            self.collected_committee_rewards
                .insert(data.committee_id, &base_fee_total);

            let request_nonce = data.request_nonce;
            data.request_nonce = request_nonce.checked_add(1).ok_or(MostError::Arithmetic)?;

            self.data.set(&data);

            // return surplus if any
            if let Some(surplus) = transferred_fee.checked_sub(current_base_fee) {
                self.env().transfer(sender, surplus)?;
            };

            self.env().emit_event(CrosschainTransferRequest {
                committee_id: data.committee_id,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
            });

            Ok(())
        }

        /// Aggregates request votes cast by guardians and mints/burns tokens
        #[ink(message)]
        pub fn receive_request(
            &mut self,
            request_hash: HashedRequest,
            committee_id: CommitteeId,
            dest_token_address: [u8; 32],
            amount: u128,
            dest_receiver_address: [u8; 32],
            request_nonce: u128,
        ) -> Result<(), MostError> {
            self.check_halted()?;

            let caller = self.env().caller();
            self.only_committee_member(committee_id, caller)?;

            let mut data = self.data()?;

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
                &committee_id.to_le_bytes(),
                &dest_token_address,
                &amount.to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.to_le_bytes(),
            ]);

            let hash = keccak256(&bytes);

            if !request_hash.eq(&hash) {
                return Err(MostError::HashDoesNotMatchData);
            }

            if self.signatures.contains((request_hash, caller)) {
                return Err(MostError::RequestAlreadySigned);
            }

            let mut request = self.pending_requests.get(request_hash).unwrap_or_default();

            // record vote
            request.signature_count = request
                .signature_count
                .checked_add(1)
                .ok_or(MostError::Arithmetic)?;
            self.signatures.insert((request_hash, caller), &());

            self.env().emit_event(RequestSigned {
                signer: caller,
                request_hash,
            });

            let signature_threshold = self
                .signature_thresholds
                .get(data.committee_id)
                .ok_or(MostError::InvalidThreshold)?;

            if request.signature_count >= signature_threshold {
                self.mint_to(
                    dest_token_address.into(),
                    dest_receiver_address.into(),
                    amount,
                )?;

                // bootstrap account with pocket money
                if data.pocket_money_balance >= data.pocket_money {
                    // don't revert if the transfer fails
                    _ = self
                        .env()
                        .transfer(dest_receiver_address.into(), data.pocket_money);
                    data.pocket_money_balance = data
                        .pocket_money_balance
                        .checked_sub(data.pocket_money)
                        .ok_or(MostError::Arithmetic)?;
                }

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
        ) -> Result<(), MostError> {
            self.check_halted()?;

            let paid_out_rewards = self.get_paid_out_member_rewards(committee_id, member_id);

            let outstanding_rewards =
                self.get_outstanding_member_rewards(committee_id, member_id)?;

            if outstanding_rewards.gt(&0) {
                self.env().transfer(member_id, outstanding_rewards)?;

                self.paid_out_member_rewards.insert(
                    (member_id, committee_id),
                    &paid_out_rewards
                        .checked_add(outstanding_rewards)
                        .ok_or(MostError::Arithmetic)?,
                );
            }

            Ok(())
        }

        /// Method used to provide the contract with funds for pocket money
        #[ink(message, payable)]
        pub fn fund_pocket_money(&mut self) -> Result<(), MostError> {
            let funds = self.env().transferred_value();
            let mut data = self.data()?;
            data.pocket_money_balance = data
                .pocket_money_balance
                .checked_add(funds)
                .ok_or(MostError::Arithmetic)?;
            self.data.set(&data);
            Ok(())
        }

        /// Upgrades contract code
        #[ink(message)]
        pub fn set_code(
            &mut self,
            code_hash: [u8; 32],
            callback: Option<Selector>,
        ) -> Result<(), MostError> {
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
                    .returns::<Result<(), MostError>>()
                    .invoke()?;
            }

            Ok(())
        }

        // ---  getter txs

        /// Query request nonce
        ///
        /// Nonce is incremented with every request
        #[ink(message)]
        pub fn get_request_nonce(&self) -> Result<u128, MostError> {
            Ok(self.data()?.request_nonce)
        }

        /// Query pocket money
        ///
        /// An amount of the native token that is tranferred with every request
        #[ink(message)]
        pub fn get_pocket_money(&self) -> Result<Balance, MostError> {
            Ok(self.data()?.pocket_money)
        }

        /// Query pocket money balance
        ///
        /// An amount of the native token that can be used for pocket money transfers
        #[ink(message)]
        pub fn get_pocket_money_balance(&self) -> Result<Balance, MostError> {
            Ok(self.data()?.pocket_money_balance)
        }

        /// Returns current active committee id
        #[ink(message)]
        pub fn get_current_committee_id(&self) -> Result<u128, MostError> {
            Ok(self.data()?.committee_id)
        }

        /// Query total rewards for this committee
        ///
        /// Denominated in AZERO
        #[ink(message)]
        pub fn get_collected_committee_rewards(&self, committee_id: CommitteeId) -> u128 {
            self.collected_committee_rewards
                .get(committee_id)
                .unwrap_or_default()
        }

        /// Query already paid out committee member rewards
        ///
        /// Denominated in AZERO
        #[ink(message)]
        pub fn get_paid_out_member_rewards(
            &self,
            committee_id: CommitteeId,
            member_id: AccountId,
        ) -> u128 {
            self.paid_out_member_rewards
                .get((member_id, committee_id))
                .unwrap_or_default()
        }

        /// Query outstanding committee member rewards
        ///
        /// The amount that can still be requested.
        /// Denominated in AZERO
        /// Returns an error (reverts) if the `member_id` account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn get_outstanding_member_rewards(
            &self,
            committee_id: CommitteeId,
            member_id: AccountId,
        ) -> Result<u128, MostError> {
            let total_amount = self
                .get_collected_committee_rewards(committee_id)
                .checked_div(
                    self.committee_sizes
                        .get(committee_id)
                        .ok_or(MostError::NotInCommittee)?,
                )
                .ok_or(MostError::Arithmetic)?;

            let collected_amount = self.get_paid_out_member_rewards(committee_id, member_id);

            Ok(total_amount.saturating_sub(collected_amount))
        }

        /// Queries a gas price oracle and returns the current base_fee charged per cross chain transfer denominated in AZERO
        #[ink(message)]
        pub fn get_base_fee(&self) -> Result<Balance, MostError> {
            if let Some(gas_price_oracle_address) = self.data()?.gas_price_oracle {
                let gas_price_oracle: contract_ref!(EthGasPriceOracle) =
                    gas_price_oracle_address.into();

                let (gas_price, timestamp) = match gas_price_oracle
                    .call()
                    .get_price()
                    .gas_limit(ORACLE_CALL_GAS_LIMIT)
                    .try_invoke()
                {
                    Ok(Ok((gas_price, timestamp))) => (gas_price, timestamp),
                    _ => return Ok(self.data()?.default_fee),
                };

                if timestamp + GAS_ORACLE_MAX_AGE < self.env().block_timestamp() {
                    return Ok(self.data()?.default_fee);
                }

                let base_fee = gas_price
                    .checked_mul(self.data()?.relay_gas_usage)
                    .ok_or(MostError::Arithmetic)?
                    .checked_mul(100u128 + BASE_FEE_BUFFER_PERCENTAGE)
                    .ok_or(MostError::Arithmetic)?
                    .checked_div(100u128)
                    .ok_or(MostError::Arithmetic)?;

                if base_fee < self.data()?.min_fee {
                    Ok(self.data()?.min_fee)
                } else if base_fee > self.data()?.max_fee {
                    Ok(self.data()?.max_fee)
                } else {
                    Ok(base_fee)
                }
            } else {
                Ok(self.data()?.default_fee)
            }
        }

        /// Returns an error (reverts) if account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn is_in_committee(&self, committee_id: CommitteeId, account: AccountId) -> bool {
            self.committees.contains((committee_id, account))
        }

        /// Returns an error (reverts) if account is not in the currently active committee
        #[ink(message)]
        pub fn only_committee_member(
            &self,
            committee_id: CommitteeId,
            account: AccountId,
        ) -> Result<(), MostError> {
            match self.is_in_committee(committee_id, account) {
                true => Ok(()),
                false => Err(MostError::NotInCommittee),
            }
        }

        // ---  setter txs

        /// Removes a supported pair from bridging
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn remove_pair(&mut self, from: [u8; 32]) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.supported_pairs.remove(from);
            Ok(())
        }

        /// Adds a supported pair for bridging
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn add_pair(&mut self, from: [u8; 32], to: [u8; 32]) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.supported_pairs.insert(from, &to);
            Ok(())
        }

        /// Sets address of the gas price oracle
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn set_gas_price_oracle(
            &mut self,
            gas_price_oracle: AccountId,
        ) -> Result<(), MostError> {
            self.ensure_owner()?;
            let mut data = self.data()?;
            data.gas_price_oracle = Some(gas_price_oracle);
            self.data.set(&data);
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
        ) -> Result<(), MostError> {
            self.ensure_owner()?;

            let mut data = self.data()?;

            if signature_threshold == 0 || committee.len().lt(&(signature_threshold as usize)) {
                return Err(MostError::InvalidThreshold);
            }

            let committee_id = data.committee_id + 1;
            let mut committee_set = Mapping::new();
            committee.into_iter().for_each(|account| {
                committee_set.insert((committee_id, account), &());
            });

            self.committees = committee_set;
            data.committee_id = committee_id;

            self.data.set(&data);

            self.signature_thresholds
                .insert(committee_id, &signature_threshold);

            Ok(())
        }

        /// Sets a new owner account
        ///
        /// Can only be called by contracts owner
        #[ink(message)]
        pub fn set_owner(&mut self, new_owner: AccountId) -> Result<(), MostError> {
            self.ensure_owner()?;
            let mut data = self.data()?;
            data.owner = new_owner;
            self.data.set(&data);
            Ok(())
        }

        /// Halt/resume the bridge contract
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn set_halted(&mut self, new_state: bool) -> Result<(), MostError> {
            self.ensure_owner()?;

            let mut data = self.data()?;
            let previous_state = data.is_halted;

            if new_state != previous_state {
                data.is_halted = new_state;

                self.data.set(&data);
                self.env().emit_event(HaltedStateChanged {
                    previous_state,
                    new_state,
                    caller: self.env().caller(),
                });
            }

            Ok(())
        }

        /// Is the bridge halted?
        #[ink(message)]
        pub fn is_halted(&self) -> Result<bool, MostError> {
            Ok(self.data()?.is_halted)
        }

        // ---  helper functions

        fn check_halted(&self) -> Result<(), MostError> {
            match self.is_halted()? {
                true => Err(MostError::IsHalted),
                false => Ok(()),
            }
        }

        fn ensure_owner(&mut self) -> Result<(), MostError> {
            let caller = self.env().caller();
            let data = self.data()?;
            match caller.eq(&data.owner) {
                true => Ok(()),
                false => Err(MostError::NotOwner(caller)),
            }
        }

        /// Mints the specified amount of token to the designated account
        ///
        /// Most contract needs to have a Minter role on the token contract
        fn mint_to(&self, token: AccountId, to: AccountId, amount: u128) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Mintable) = token.into();
            psp22.mint(to, amount)
        }

        /// Burn the specified amount of token from the designated account
        ///
        /// Most contract needs to have a Burner role on the token contract
        fn burn_from(
            &self,
            token: AccountId,
            from: AccountId,
            amount: u128,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Burnable) = token.into();
            psp22.burn_from(from, amount)
        }

        fn data(&self) -> Result<Data, MostError> {
            self.data.get().ok_or(MostError::CorruptedStorage)
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
        const POCKET_MONEY: Balance = 1000000000000;
        const RELAY_GAS_USAGE: u128 = 50000;
        const MIN_FEE: Balance = 1000000000000;
        const MAX_FEE: Balance = 100000000000000;
        const DEFAULT_FEE: Balance = 30000000000000;

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
                Most::new(
                    guardian_accounts(),
                    0,
                    POCKET_MONEY,
                    RELAY_GAS_USAGE,
                    MIN_FEE,
                    MAX_FEE,
                    DEFAULT_FEE,
                    None,
                )
                .expect_err("Threshold is zero, instantiation should fail."),
                MostError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_fails_on_threshold_large_than_guardians() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            assert_eq!(
                Most::new(
                    guardian_accounts(),
                    (guardian_accounts().len() + 1) as u128,
                    POCKET_MONEY,
                    RELAY_GAS_USAGE,
                    MIN_FEE,
                    MAX_FEE,
                    DEFAULT_FEE,
                    None,
                )
                .expect_err("Threshold is larger than guardians, instantiation should fail."),
                MostError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_sets_caller_as_owner() {
            set_caller::<DefEnv>(default_accounts::<DefEnv>().alice);
            let mut most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                None,
            )
            .expect("Threshold is valid.");

            assert_eq!(most.ensure_owner(), Ok(()));
            set_caller::<DefEnv>(guardian_accounts()[0]);
            assert_eq!(
                most.ensure_owner(),
                Err(MostError::NotOwner(guardian_accounts()[0]))
            );
        }

        #[ink::test]
        fn new_sets_correct_guardians() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                None,
            )
            .expect("Threshold is valid.");

            for account in guardian_accounts() {
                assert!(most.is_in_committee(most.get_current_committee_id().unwrap(), account));
            }
            assert!(!most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.alice));
        }

        #[ink::test]
        fn set_owner_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                None,
            )
            .expect("Threshold is valid.");
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(most.ensure_owner(), Err(MostError::NotOwner(accounts.bob)));
            set_caller::<DefEnv>(accounts.alice);
            assert_eq!(most.ensure_owner(), Ok(()));
            assert_eq!(most.set_owner(accounts.bob), Ok(()));
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(most.ensure_owner(), Ok(()));
        }

        #[ink::test]
        fn add_guardian_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                None,
            )
            .expect("Threshold is valid.");

            assert!(!most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.alice));
            assert_eq!(most.set_committee(vec![accounts.alice], 1), Ok(()));
            assert!(most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.alice));
        }

        #[ink::test]
        fn remove_guardian_works() {
            let accounts = default_accounts::<DefEnv>();
            set_caller::<DefEnv>(accounts.alice);
            let mut most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                None,
            )
            .expect("Threshold is valid.");

            assert!(most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.bob));
            assert_eq!(most.set_committee(vec![accounts.alice], 1), Ok(()));
            assert!(!most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.bob));
        }
    }
}
