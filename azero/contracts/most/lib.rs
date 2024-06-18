#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use ownable2step::Ownable2StepError;

pub use self::most::{MostError, MostRef};

#[ink::contract]
pub mod most {

    use gas_oracle_trait::EthGasPriceOracle;
    use ink::{
        codegen::TraitCallBuilder,
        contract_ref,
        env::{set_code_hash, Error as InkEnvError},
        prelude::{format, string::String, vec, vec::Vec},
        storage::{traits::ManualKey, Lazy, Mapping},
    };
    use ownable2step::*;
    use psp22::{PSP22Error, PSP22};
    use psp22_traits::{Burnable, Mintable, WrappedAZERO};
    use scale::{Decode, Encode};
    use shared::{hash_request_data, Keccak256HashOutput as HashedRequest};

    type CommitteeId = u128;

    const ZERO_ADDRESS: [u8; 32] = [0; 32];
    const NATIVE_MARKER_ADDRESS: [u8; 32] = [0; 32];

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

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct TransferOwnershipInitiated {
        pub new_owner: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct TransferOwnershipAccepted {
        pub new_owner: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct RequestAlreadySigned {
        pub request_hash: HashedRequest,
        #[ink(topic)]
        pub signer: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct PayoutAccountSet {
        #[ink(topic)]
        pub member: AccountId,
        #[ink(topic)]
        pub account: AccountId,
    }

    #[derive(Default, Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Request {
        signature_count: u128,
    }

    #[derive(Debug, Encode, Decode, Clone, Copy)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum RequestStatus {
        Pending { collected_signatures: u32 },
        Processed,
        RequestHashNotKnown,
    }

    #[derive(Debug)]
    #[ink::storage_item]
    pub struct Data {
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
        /// minimum gas price used to calculate the fee that can be charged for a cross-chain transfer request
        min_gas_price: u128,
        /// maximum gas price used to calculate the fee that can be charged for a cross-chain transfer request
        max_gas_price: u128,
        /// default gas price used to calculate the fee that is charged for a cross-chain transfer request if the gas price oracle is not available/malfunctioning
        default_gas_price: u128,
        /// maximum time in milliseconds since last oracle update
        gas_oracle_max_age: u64,
        /// maximum limit on gas for a call to the oracle
        oracle_call_gas_limit: u64,
        /// percentage buffer over the estimated cost
        base_fee_buffer_percentage: u128,
        /// gas price oracle that is used to calculate the fee for a cross-chain transfer request
        gas_price_oracle: Option<AccountId>,
        /// Is the bridge in a halted state
        is_halted: bool,
    }

    #[ink(storage)]
    pub struct Most {
        data: Lazy<Data, ManualKey<0x44415441>>,
        /// stores the data used by the Ownable2Step trait
        ownable_data: Lazy<Ownable2StepData, ManualKey<0xDEADBEEF>>,
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
        /// committe members can specify a special account for collecting the rewards, different from the one used for signing
        payout_accounts: Mapping<AccountId, AccountId, ManualKey<0x18f97340>>,
        /// Wrapped ethereum (azero) address. Necessary to perform bridging weth(azero) -> ether(eth).
        weth: Lazy<AccountId, ManualKey<0x7CD95FED>>,
        /// Mapping holding the information wheter a specific token originates on Aleph Zero chain
        local_token: Mapping<AccountId, (), ManualKey<0x6c6f6361>>,
        /// Wrapped AZERO address
        wazero: Lazy<AccountId, ManualKey<0x77617a65>>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MostError {
        Constructor,
        InvalidThreshold,
        DuplicateCommitteeMember,
        ZeroTransferAmount,
        NotInCommittee,
        NoSuchCommittee,
        HashDoesNotMatchData,
        PSP22(PSP22Error),
        Ownable(Ownable2StepError),
        UnsupportedPair,
        InkEnvError(String),
        RequestAlreadySigned,
        BaseFeeTooLow,
        Arithmetic,
        CorruptedStorage,
        IsHalted,
        HaltRequired,
        NoMintPermission,
        ZeroAddress,
        WrappedEthNotSet,
        WrappedAzeroNotSet,
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

    impl From<Ownable2StepError> for MostError {
        fn from(inner: Ownable2StepError) -> Self {
            MostError::Ownable(inner)
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
            min_gas_price: u128,
            max_gas_price: u128,
            default_gas_price: u128,
            gas_oracle_max_age: u64,
            oracle_call_gas_limit: u64,
            base_fee_buffer_percentage: u128,
            gas_price_oracle: Option<AccountId>,
            owner: AccountId,
        ) -> Result<Self, MostError> {
            Self::check_committee(&committee, signature_threshold)?;

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
                request_nonce: 0,
                committee_id,
                pocket_money,
                pocket_money_balance: 0,
                relay_gas_usage,
                min_gas_price,
                max_gas_price,
                default_gas_price,
                gas_oracle_max_age,
                oracle_call_gas_limit,
                base_fee_buffer_percentage,
                gas_price_oracle,
                is_halted: true,
            });

            let mut ownable_data = Lazy::new();
            ownable_data.set(&Ownable2StepData::new(owner));
            let weth = Lazy::new();
            let wazero = Lazy::new();

            Ok(Self {
                data,
                ownable_data,
                signature_thresholds,
                committees,
                committee_sizes,
                pending_requests: Mapping::new(),
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                supported_pairs: Mapping::new(),
                collected_committee_rewards: Mapping::new(),
                paid_out_member_rewards: Mapping::new(),
                payout_accounts: Mapping::new(),
                weth,
                local_token: Mapping::new(),
                wazero,
            })
        }

        // --- business logic
        fn _send_request(
            &mut self,
            src_token_address: AccountId,
            dest_token_address: [u8; 32],
            amount: u128,
            dest_receiver_address: [u8; 32],
            native_azero_request: bool,
            transferred_fee: u128,
        ) -> Result<(), MostError> {
            if dest_receiver_address == ZERO_ADDRESS {
                return Err(MostError::ZeroAddress);
            }

            if amount == 0 {
                return Err(MostError::ZeroTransferAmount);
            }

            let current_base_fee = self.get_base_fee()?;

            if transferred_fee.lt(&current_base_fee) {
                return Err(MostError::BaseFeeTooLow);
            }

            let sender = self.env().caller();

            if self.local_token.get(src_token_address).is_none() {
                self.burn_from(src_token_address, sender, amount)?;
            } else if !native_azero_request {
                // lock the tokens in the contract
                self.transfer_from(src_token_address, sender, amount)?;
            }

            let mut data = self.data()?;
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
                if surplus > 0 {
                    self.env().transfer(sender, surplus)?;
                }
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
            self.ensure_not_halted()?;

            let dest_token_address = self
                .supported_pairs
                .get(src_token_address)
                .ok_or(MostError::UnsupportedPair)?;

            self._send_request(
                src_token_address.into(),
                dest_token_address,
                amount,
                dest_receiver_address,
                false,
                self.env().transferred_value(),
            )
        }

        /// Invoke this tx to initiate weth -> ether transfer to the destination chain.
        ///
        /// Upon checking basic conditions the contract will burn the `amount` number of weth tokens from the caller
        /// and emit an event which is to be picked up & acted on up by the bridge guardians.
        #[ink(message, payable)]
        pub fn send_request_ether_to_native(
            &mut self,
            amount: u128,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MostError> {
            self.ensure_not_halted()?;

            let src_token_address = self.weth.get().ok_or(MostError::WrappedEthNotSet)?;

            // ETH_ZERO_ADDRESS as `dest_token_address` indicates native ether transfer
            self._send_request(
                src_token_address,
                NATIVE_MARKER_ADDRESS,
                amount,
                dest_receiver_address,
                false,
                self.env().transferred_value(),
            )
        }

        #[ink(message, payable)]
        pub fn send_request_native_azero(
            &mut self,
            amount: u128,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MostError> {
            self.ensure_not_halted()?;

            let wrapped_azero_address = self.wazero.get().ok_or(MostError::WrappedAzeroNotSet)?;
            let wrapped_azero_address_bytes: [u8; 32] = *wrapped_azero_address.as_ref();
            let mut wrapped_azero: contract_ref!(WrappedAZERO) = wrapped_azero_address.into();

            wrapped_azero
                .call_mut()
                .deposit()
                .transferred_value(amount)
                .invoke()?;

            let dest_token_address = self
                .supported_pairs
                .get(wrapped_azero_address_bytes)
                .ok_or(MostError::UnsupportedPair)?;

            self._send_request(
                wrapped_azero_address,
                dest_token_address,
                amount,
                dest_receiver_address,
                true,
                self.env().transferred_value() - amount,
            )
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
            self.ensure_not_halted()?;

            let caller = self.env().caller();
            self.only_committee_member(committee_id, caller)?;

            // Don't revert if the request has already been processed as
            // such a call can be made during regular guardian operation.
            if self.processed_requests.contains(request_hash) {
                self.env().emit_event(SignedProcessedRequest {
                    request_hash,
                    signer: caller,
                });
                return Ok(());
            }

            if self.signatures.contains((request_hash, caller)) {
                self.env().emit_event(RequestAlreadySigned {
                    request_hash,
                    signer: caller,
                });
                return Ok(());
            }

            let hash = hash_request_data(
                committee_id,
                dest_token_address.into(),
                amount,
                dest_receiver_address.into(),
                request_nonce,
            );

            if !request_hash.eq(&hash) {
                return Err(MostError::HashDoesNotMatchData);
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
                .get(committee_id)
                .ok_or(MostError::InvalidThreshold)?;

            if request.signature_count >= signature_threshold {
                let is_local_token = self
                    .local_token
                    .contains::<AccountId>(dest_token_address.into());

                if dest_token_address == NATIVE_MARKER_ADDRESS {
                    self.unwrap_azero_to(dest_receiver_address.into(), amount)?;
                } else if is_local_token {
                    self.transfer(
                        dest_token_address.into(),
                        dest_receiver_address.into(),
                        amount,
                    )?;
                } else {
                    self.mint_to(
                        dest_token_address.into(),
                        dest_receiver_address.into(),
                        amount,
                    )?;
                }

                let mut data = self.data()?;
                // bootstrap account with pocket money
                if data.pocket_money_balance >= data.pocket_money {
                    // don't revert if the transfer fails
                    if self
                        .env()
                        .transfer(dest_receiver_address.into(), data.pocket_money)
                        .is_ok()
                    {
                        data.pocket_money_balance = data
                            .pocket_money_balance
                            .checked_sub(data.pocket_money)
                            .ok_or(MostError::Arithmetic)?;
                        self.data.set(&data);
                    }
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
        /// Reverts if `member_id` account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn payout_rewards(
            &mut self,
            committee_id: CommitteeId,
            member_id: AccountId,
        ) -> Result<(), MostError> {
            self.ensure_not_halted()?;
            self.only_committee_member(committee_id, member_id)?;

            let paid_out_rewards = self.get_paid_out_member_rewards(committee_id, member_id);

            let outstanding_rewards =
                self.get_outstanding_member_rewards(committee_id, member_id)?;

            if outstanding_rewards.gt(&0) {
                let payout_account = self.payout_accounts.get(member_id).unwrap_or(member_id);
                self.env().transfer(payout_account, outstanding_rewards)?;

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
        pub fn set_code(&mut self, code_hash: [u8; 32]) -> Result<(), MostError> {
            self.ensure_owner()?;
            set_code_hash(&code_hash)?;
            Ok(())
        }

        // --- getters

        /// Query average gas it takes for a confirmation of a single cross-chain transfer request on the destination chain
        ///
        /// Sum of the total gas usage of *all* the relay transactions divided it by the current committee size and multiplied by 1.2
        #[ink(message)]
        pub fn get_relay_gas_usage(&self) -> Result<u128, MostError> {
            Ok(self.data()?.relay_gas_usage)
        }

        /// Query minimum gas price used to calculate the fee that can be charged for a cross-chain transfer request
        #[ink(message)]
        pub fn get_min_gas_price(&self) -> Result<u128, MostError> {
            Ok(self.data()?.min_gas_price)
        }

        /// Query maximum gas price used to calculate the fee that can be charged for a cross-chain transfer request
        #[ink(message)]
        pub fn get_max_gas_price(&self) -> Result<u128, MostError> {
            Ok(self.data()?.max_gas_price)
        }

        /// Query default gas price used to calculate the fee that is charged for a cross-chain transfer request if the gas price oracle is not available
        #[ink(message)]
        pub fn get_default_gas_price(&self) -> Result<u128, MostError> {
            Ok(self.data()?.default_gas_price)
        }

        /// Query maximum time in milliseconds since last oracle update
        #[ink(message)]
        pub fn get_gas_oracle_max_age(&self) -> Result<u64, MostError> {
            Ok(self.data()?.gas_oracle_max_age)
        }

        /// Query maximum limit on gas for a call to the oracle
        #[ink(message)]
        pub fn get_oracle_call_gas_limit(&self) -> Result<u64, MostError> {
            Ok(self.data()?.oracle_call_gas_limit)
        }

        /// Query percentage buffer over the estimated cost
        #[ink(message)]
        pub fn get_base_fee_buffer_percentage(&self) -> Result<u128, MostError> {
            Ok(self.data()?.base_fee_buffer_percentage)
        }

        /// Query gas price oracle address
        #[ink(message)]
        pub fn get_gas_price_oracle(&self) -> Result<Option<AccountId>, MostError> {
            Ok(self.data()?.gas_price_oracle)
        }

        /// Query token pair
        #[ink(message)]
        pub fn get_supported_pair(&self, src_token: [u8; 32]) -> Option<[u8; 32]> {
            self.supported_pairs.get(src_token)
        }

        /// Query payout_account for a committee member (if any)
        #[ink(message)]
        pub fn get_payout_account(&self, member_id: AccountId) -> Option<AccountId> {
            self.payout_accounts.get(member_id)
        }

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
        /// Returns 0 if the `member_id` account is not in the committee with `committee_id`
        #[ink(message)]
        pub fn get_outstanding_member_rewards(
            &self,
            committee_id: CommitteeId,
            member_id: AccountId,
        ) -> Result<u128, MostError> {
            if self.is_in_committee(committee_id, member_id) {
                let total_amount = self
                    .get_collected_committee_rewards(committee_id)
                    .checked_div(
                        self.committee_sizes
                            .get(committee_id)
                            .ok_or(MostError::NoSuchCommittee)?,
                    )
                    .ok_or(MostError::Arithmetic)?;

                let collected_amount = self.get_paid_out_member_rewards(committee_id, member_id);

                return Ok(total_amount.saturating_sub(collected_amount));
            }
            Ok(0)
        }

        /// Queries a gas price oracle and returns the current base_fee charged per cross chain transfer denominated in AZERO
        #[ink(message)]
        pub fn get_base_fee(&self) -> Result<Balance, MostError> {
            let gas_price = if let Some(gas_price_oracle_address) = self.data()?.gas_price_oracle {
                let gas_price_oracle: contract_ref!(EthGasPriceOracle) =
                    gas_price_oracle_address.into();

                match gas_price_oracle
                    .call()
                    .get_price()
                    .gas_limit(self.data()?.oracle_call_gas_limit)
                    .try_invoke()
                {
                    Ok(Ok((gas_price, timestamp))) => {
                        if timestamp + self.data()?.gas_oracle_max_age
                            < self.env().block_timestamp()
                        {
                            self.data()?.default_gas_price
                        } else if gas_price < self.data()?.min_gas_price {
                            self.data()?.min_gas_price
                        } else if gas_price > self.data()?.max_gas_price {
                            self.data()?.max_gas_price
                        } else {
                            gas_price
                        }
                    }
                    _ => self.data()?.default_gas_price,
                }
            } else {
                self.data()?.default_gas_price
            };

            let base_fee = gas_price
                .checked_mul(self.data()?.relay_gas_usage)
                .ok_or(MostError::Arithmetic)?
                .checked_mul(100u128 + self.data()?.base_fee_buffer_percentage)
                .ok_or(MostError::Arithmetic)?
                .checked_div(100u128)
                .ok_or(MostError::Arithmetic)?;
            Ok(base_fee)
        }

        /// Returns whether an account is in the committee with `committee_id`
        #[ink(message)]
        pub fn is_in_committee(&self, committee_id: CommitteeId, account: AccountId) -> bool {
            self.committees.contains((committee_id, account))
        }

        /// Returns `true` if the account has signed the request with hash `request_hash`
        #[ink(message)]
        pub fn has_signed_request(&self, account: AccountId, request_hash: HashedRequest) -> bool {
            self.signatures.contains((request_hash, account))
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

        #[ink(message)]
        pub fn needs_signature(
            &self,
            request_hash: HashedRequest,
            account: AccountId,
            committee_id: CommitteeId,
        ) -> bool {
            if !self.is_in_committee(committee_id, account) {
                // signer not in the current committee
                return false;
            }

            if self.processed_requests.contains(request_hash) {
                // request already processed
                return false;
            }

            if self.has_signed_request(account, request_hash) {
                // request already signed
                return false;
            }
            true
        }

        /// Is the bridge halted?
        #[ink(message)]
        pub fn is_halted(&self) -> Result<bool, MostError> {
            Ok(self.data()?.is_halted)
        }

        /// Returns the status of a given cross-chain transfer request
        #[ink(message)]
        pub fn request_status(&self, hashed_request: HashedRequest) -> RequestStatus {
            if self.processed_requests.contains(hashed_request) {
                RequestStatus::Processed
            } else if let Some(Request { signature_count }) =
                self.pending_requests.get(hashed_request)
            {
                RequestStatus::Pending {
                    collected_signatures: signature_count as u32,
                }
            } else {
                RequestStatus::RequestHashNotKnown
            }
        }

        // ---  setter txs

        #[allow(clippy::too_many_arguments)]
        #[ink(message)]
        pub fn set_base_fee_constraints(
            &mut self,
            relay_gas_usage: u128,
            min_gas_price: u128,
            max_gas_price: u128,
            default_gas_price: u128,
            gas_oracle_max_age: u64,
            oracle_call_gas_limit: u64,
            base_fee_buffer_percentage: u128,
        ) -> Result<(), MostError> {
            self.ensure_owner()?;
            let mut data = self.data()?;
            data.relay_gas_usage = relay_gas_usage;
            data.min_gas_price = min_gas_price;
            data.max_gas_price = max_gas_price;
            data.default_gas_price = default_gas_price;
            data.gas_oracle_max_age = gas_oracle_max_age;
            data.oracle_call_gas_limit = oracle_call_gas_limit;
            data.base_fee_buffer_percentage = base_fee_buffer_percentage;
            self.data.set(&data);
            Ok(())
        }

        /// Removes a supported pair from bridging
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn remove_pair(&mut self, from: [u8; 32]) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.ensure_halted()?;
            self.supported_pairs.remove(from);
            Ok(())
        }

        /// Adds a supported pair for bridging
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn add_pair(
            &mut self,
            from: [u8; 32],
            to: [u8; 32],
            local_token: bool,
        ) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.ensure_halted()?;

            // Check if MOST has mint permission to the PSP22 token
            if local_token {
                self.local_token.insert::<AccountId, ()>(from.into(), &());
            } else {
                let psp22_address: AccountId = from.into();
                let psp22: ink::contract_ref!(Mintable) = psp22_address.into();
                if psp22.minter() != self.env().account_id() {
                    return Err(MostError::NoMintPermission);
                }
            }

            self.supported_pairs.insert(from, &to);
            Ok(())
        }

        /// Set weth(azero) psp22 token contract
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn set_weth(&mut self, weth_address: AccountId) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.weth.set(&weth_address);
            Ok(())
        }

        /// Set wazero(azero) psp22 token contract
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn set_wazero(&mut self, wazero_address: AccountId) -> Result<(), MostError> {
            self.ensure_owner()?;
            self.ensure_halted()?;
            self.wazero.set(&wazero_address);
            Ok(())
        }

        /// Sets address of the account that receives rewards on behalf of the committee member.
        ///
        /// Can only be called by an account that was a committee member in `committee_id`.
        /// All the unpaid rewards will then be paid out to this account address when `payout_request` is called.
        /// Comittee members that have set the `payout_account` do not need to call this method again when the committee changes, unless they want to change the payouts account.
        #[ink(message)]
        pub fn set_payout_account(
            &mut self,
            committee_id: CommitteeId,
            account: AccountId,
        ) -> Result<(), MostError> {
            let caller = self.env().caller();
            self.only_committee_member(committee_id, caller)?;
            self.payout_accounts.insert(caller, &account);

            self.env().emit_event(PayoutAccountSet {
                member: caller,
                account,
            });

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
            self.ensure_halted()?;
            Self::check_committee(&committee, signature_threshold)?;

            let mut data = self.data()?;

            let committee_id = data.committee_id + 1;
            self.committee_sizes
                .insert(committee_id, &(committee.len() as u128));
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

        /// Transfer PSP22 tokens from the bridge contract to a given account.
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn recover_psp22(
            &mut self,
            token: AccountId,
            receiver: AccountId,
            amount: u128,
        ) -> Result<(), MostError> {
            self.ensure_owner()?;

            let mut token: ink::contract_ref!(PSP22) = token.into();
            token.transfer(receiver, amount, vec![])?;
            Ok(())
        }

        /// Transfer AZERO tokens from the bridge contract to a given account.
        ///
        /// Can only be called by the contracts owner
        #[ink(message)]
        pub fn recover_azero(
            &mut self,
            receiver: AccountId,
            amount: u128,
        ) -> Result<(), MostError> {
            self.ensure_owner()?;

            self.env().transfer(receiver, amount)?;
            Ok(())
        }

        // ---  helper functions

        fn ensure_halted(&self) -> Result<(), MostError> {
            match self.is_halted()? {
                true => Ok(()),
                false => Err(MostError::HaltRequired),
            }
        }

        fn ensure_not_halted(&self) -> Result<(), MostError> {
            match self.is_halted()? {
                true => Err(MostError::IsHalted),
                false => Ok(()),
            }
        }

        fn check_committee(committee: &[AccountId], threshold: u128) -> Result<(), MostError> {
            if threshold == 0 || committee.len().lt(&(threshold as usize)) {
                return Err(MostError::InvalidThreshold);
            }

            for i in 0..committee.len() {
                for j in i + 1..committee.len() {
                    if committee[i] == committee[j] {
                        return Err(MostError::DuplicateCommitteeMember);
                    }
                }
            }
            Ok(())
        }

        /// Mints the specified amount of token to the designated account
        ///
        /// Most contract needs to have a Minter role on the token contract
        fn mint_to(&self, token: AccountId, to: AccountId, amount: u128) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(Mintable) = token.into();
            psp22.mint(to, amount)
        }

        /// Transfers the specified amount of token to the designated account
        fn transfer(
            &self,
            token: AccountId,
            to: AccountId,
            amount: u128,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(PSP22) = token.into();
            psp22.transfer(to, amount, vec![])
        }

        /// Unwraps the specified amount of AZERO locked in the bridge contract and transfers it to the designated account
        fn unwrap_azero_to(&self, to: AccountId, amount: u128) -> Result<(), MostError> {
            let wrapped_azero_address = self.wazero.get().ok_or(MostError::WrappedAzeroNotSet)?;
            let mut wrapped_azero: ink::contract_ref!(WrappedAZERO) = wrapped_azero_address.into();

            wrapped_azero.withdraw(amount)?;
            self.env().transfer(to, amount)?;
            Ok(())
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
            self.transfer_from(token, from, amount)?;

            let mut psp22_burnable: ink::contract_ref!(Burnable) = token.into();
            psp22_burnable.burn(amount)
        }

        /// Transfers the specified amount of token from the designated account
        fn transfer_from(
            &self,
            token: AccountId,
            from: AccountId,
            amount: u128,
        ) -> Result<(), PSP22Error> {
            let mut psp22: ink::contract_ref!(PSP22) = token.into();
            psp22.transfer_from(from, self.env().account_id(), amount, vec![])
        }

        fn data(&self) -> Result<Data, MostError> {
            self.data.get().ok_or(MostError::CorruptedStorage)
        }

        fn ownable_data(&self) -> Result<Ownable2StepData, Ownable2StepError> {
            self.ownable_data
                .get()
                .ok_or(Ownable2StepError::Custom("CorruptedStorage".into()))
        }
    }

    impl Ownable2Step for Most {
        #[ink(message)]
        fn get_owner(&self) -> Ownable2StepResult<AccountId> {
            self.ownable_data()?.get_owner()
        }

        #[ink(message)]
        fn get_pending_owner(&self) -> Ownable2StepResult<AccountId> {
            self.ownable_data()?.get_pending_owner()
        }

        #[ink(message)]
        fn transfer_ownership(&mut self, new_owner: AccountId) -> Ownable2StepResult<()> {
            let mut ownable_data = self.ownable_data()?;
            ownable_data.transfer_ownership(self.env().caller(), new_owner)?;
            self.ownable_data.set(&ownable_data);
            self.env()
                .emit_event(TransferOwnershipInitiated { new_owner });
            Ok(())
        }

        #[ink(message)]
        fn accept_ownership(&mut self) -> Ownable2StepResult<()> {
            let new_owner = self.env().caller();
            let mut ownable_data = self.ownable_data()?;
            ownable_data.accept_ownership(new_owner)?;
            self.ownable_data.set(&ownable_data);
            self.env()
                .emit_event(TransferOwnershipAccepted { new_owner });
            Ok(())
        }

        #[ink(message)]
        fn ensure_owner(&self) -> Ownable2StepResult<()> {
            self.ownable_data()?.ensure_owner(self.env().caller())
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
        const GAS_ORACLE_MAX_AGE: u64 = 86400000;
        const ORACLE_CALL_GAS_LIMIT: u64 = 2000000000;
        const BASE_FEE_BUFFER_PERCENTAGE: u128 = 20;

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
            let alice = default_accounts::<DefEnv>().alice;
            set_caller::<DefEnv>(alice);

            assert_eq!(
                Most::new(
                    guardian_accounts(),
                    0,
                    POCKET_MONEY,
                    RELAY_GAS_USAGE,
                    MIN_FEE,
                    MAX_FEE,
                    DEFAULT_FEE,
                    GAS_ORACLE_MAX_AGE,
                    ORACLE_CALL_GAS_LIMIT,
                    BASE_FEE_BUFFER_PERCENTAGE,
                    None,
                    alice
                )
                .expect_err("Threshold is zero, instantiation should fail."),
                MostError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_fails_on_threshold_large_than_guardians() {
            let alice = default_accounts::<DefEnv>().alice;
            set_caller::<DefEnv>(alice);

            assert_eq!(
                Most::new(
                    guardian_accounts(),
                    (guardian_accounts().len() + 1) as u128,
                    POCKET_MONEY,
                    RELAY_GAS_USAGE,
                    MIN_FEE,
                    MAX_FEE,
                    DEFAULT_FEE,
                    GAS_ORACLE_MAX_AGE,
                    ORACLE_CALL_GAS_LIMIT,
                    BASE_FEE_BUFFER_PERCENTAGE,
                    None,
                    alice
                )
                .expect_err("Threshold is larger than guardians, instantiation should fail."),
                MostError::InvalidThreshold
            );
        }

        #[ink::test]
        fn new_sets_caller_as_owner() {
            let alice = default_accounts::<DefEnv>().alice;
            set_caller::<DefEnv>(alice);

            let most = Most::new(
                guardian_accounts(),
                THRESHOLD,
                POCKET_MONEY,
                RELAY_GAS_USAGE,
                MIN_FEE,
                MAX_FEE,
                DEFAULT_FEE,
                GAS_ORACLE_MAX_AGE,
                ORACLE_CALL_GAS_LIMIT,
                BASE_FEE_BUFFER_PERCENTAGE,
                None,
                alice,
            )
            .expect("Threshold is valid.");

            assert_eq!(most.ensure_owner(), Ok(()));
            set_caller::<DefEnv>(guardian_accounts()[0]);
            assert_eq!(
                most.ensure_owner(),
                Err(Ownable2StepError::CallerNotOwner(guardian_accounts()[0]))
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
                GAS_ORACLE_MAX_AGE,
                ORACLE_CALL_GAS_LIMIT,
                BASE_FEE_BUFFER_PERCENTAGE,
                None,
                accounts.alice,
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
                GAS_ORACLE_MAX_AGE,
                ORACLE_CALL_GAS_LIMIT,
                BASE_FEE_BUFFER_PERCENTAGE,
                None,
                accounts.alice,
            )
            .expect("Threshold is valid.");
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(
                most.ensure_owner(),
                Err(Ownable2StepError::CallerNotOwner(accounts.bob))
            );
            set_caller::<DefEnv>(accounts.alice);
            assert_eq!(most.ensure_owner(), Ok(()));
            assert_eq!(most.transfer_ownership(accounts.bob), Ok(()));
            // check that bob has to accept before being granted ownership
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(
                most.ensure_owner(),
                Err(Ownable2StepError::CallerNotOwner(accounts.bob))
            );
            // check that only bob can accept the pending new ownership
            set_caller::<DefEnv>(accounts.charlie);
            assert_eq!(
                most.accept_ownership(),
                Err(Ownable2StepError::CallerNotPendingOwner(accounts.charlie))
            );
            set_caller::<DefEnv>(accounts.bob);
            assert_eq!(most.accept_ownership(), Ok(()));
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
                GAS_ORACLE_MAX_AGE,
                ORACLE_CALL_GAS_LIMIT,
                BASE_FEE_BUFFER_PERCENTAGE,
                None,
                accounts.alice,
            )
            .expect("Threshold is valid.");

            assert!(!most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.alice));
            assert_eq!(most.committee_sizes.get(0), Some(5));
            assert_eq!(most.set_committee(vec![accounts.alice], 1), Ok(()));
            assert_eq!(most.committee_sizes.get(1), Some(1));
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
                GAS_ORACLE_MAX_AGE,
                ORACLE_CALL_GAS_LIMIT,
                BASE_FEE_BUFFER_PERCENTAGE,
                None,
                accounts.alice,
            )
            .expect("Threshold is valid.");

            assert!(most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.bob));
            assert_eq!(most.committee_sizes.get(0), Some(5));
            assert_eq!(most.set_committee(vec![accounts.alice], 1), Ok(()));
            assert_eq!(most.committee_sizes.get(1), Some(1));
            assert!(!most.is_in_committee(most.get_current_committee_id().unwrap(), accounts.bob));
        }
    }
}
