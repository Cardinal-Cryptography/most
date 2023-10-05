#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod membrane {
    use ink::{
        codegen::EmitEvent,
        prelude::{format, string::String, vec, vec::Vec},
        reflect::ContractEventBase,
        storage::{traits::ManualKey, Mapping},
    };
    use psp22_traits::{PSP22Error, PSP22};
    use scale::{Decode, Encode};

    #[ink(event)]
    #[derive(Debug)]
    pub struct CrosschainTransferRequest {
        sender: [u8; 32],
        src_token_address: [u8; 32],
        src_token_amount: u128,
        dest_chain_id: u32,
        dest_token_address: [u8; 32],
        dest_token_amount: u128,
        dest_receiver_address: [u8; 32],
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RequestProcessed {}

    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Request {
        dest_token_address: AccountId,
        dest_token_amount: Balance,
        dest_receiver_address: AccountId,
        signature_count: u128,
    }

    #[ink(storage)]
    pub struct Membrane {
        request_nonce: u128,
        signature_threshold: u128,
        pending_requests: Mapping<[u8; 32], Request>,
        request_signatures: Mapping<([u8; 32], AccountId), ()>,
        processed_requests: Mapping<[u8; 32], ()>,
        guardians: Mapping<AccountId, ()>,
    }

    pub type Event = <Membrane as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        NotGuardian,
        PSP22(PSP22Error),
    }

    impl From<PSP22Error> for MembraneError {
        fn from(inner: PSP22Error) -> Self {
            MembraneError::PSP22(inner)
        }
    }

    impl Membrane {
        #[ink(constructor)]
        pub fn new(guardians: Vec<AccountId>, signature_threshold: u128) -> Self {
            let mut guardians_set = Mapping::new();
            guardians.into_iter().for_each(|account| {
                guardians_set.insert(account, &());
            });

            Self {
                request_nonce: 0,
                signature_threshold,
                pending_requests: Mapping::new(),
                request_signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                guardians: guardians_set,
            }
        }

        /// Invoke this tx to transfer funds to the destination chain.
        #[ink(message)]
        pub fn send_request(
            &mut self,
            src_token_address: AccountId,
            src_token_amount: Balance,
            dest_chain_id: u32,
            dest_token_address: [u8; 32],
            dest_token_amount: u128,
            dest_receiver_address: [u8; 32],
        ) -> Result<(), MembraneError> {
            let sender = self.env().caller();

            self.transfer_from_tx(
                src_token_address,
                sender,
                self.env().account_id(),
                src_token_amount,
            )?;

            Self::emit_event(
                self.env(),
                Event::CrosschainTransferRequest(CrosschainTransferRequest {
                    sender: *sender.as_ref(),
                    src_token_address: *src_token_address.as_ref(),
                    src_token_amount,
                    dest_chain_id,
                    dest_token_address,
                    dest_token_amount,
                    dest_receiver_address,
                }),
            );

            self.request_nonce += 1;

            Ok(())
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

        fn is_guardian(&self, account: AccountId) -> Result<(), MembraneError> {
            if self.guardians.contains(account) {
                Ok(())
            } else {
                Err(MembraneError::NotGuardian)
            }
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
