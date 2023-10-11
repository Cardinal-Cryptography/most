#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod membrane {
    use ink::{
        codegen::EmitEvent,
        env::{
            hash::{HashOutput, Keccak256},
            hash_bytes,
        },
        prelude::{vec, vec::Vec},
        reflect::ContractEventBase,
        storage::Mapping,
    };
    use psp22_traits::{Mintable, PSP22Error, PSP22};
    use scale::{Decode, Encode};

    #[ink(event)]
    #[derive(Debug)]
    pub struct CrosschainTransferRequest {
        sender: [u8; 32],
        src_token_address: [u8; 32],
        src_token_amount: u128,
        dest_token_address: [u8; 32],
        dest_token_amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct RequestProcessed {
        request_hash: [u8; 32],
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct SignatureTallied {
        signer: AccountId,
        request_hash: [u8; 32],
    }

    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Request {
        dest_token_address: [u8; 32],
        dest_token_amount: u128,
        dest_receiver_address: [u8; 32],
        signature_count: u128,
    }

    #[ink(storage)]
    pub struct Membrane {
        request_nonce: u128,
        signature_threshold: u128,
        pending_requests: Mapping<[u8; 32], Request>,
        signatures: Mapping<([u8; 32], AccountId), ()>,
        processed_requests: Mapping<[u8; 32], ()>,
        guardians: Mapping<AccountId, ()>,
    }

    pub type Event = <Membrane as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MembraneError {
        NotGuardian,
        HashDoesNotMatchData,
        PSP22(PSP22Error),
        RequestAlreadyProcessed,
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
                signatures: Mapping::new(),
                processed_requests: Mapping::new(),
                guardians: guardians_set,
            }
        }

        /// Invoke this tx to initiate funds transfer to the destination chain.
        #[ink(message)]
        pub fn send_request(
            &mut self,
            src_token_address: AccountId,
            src_token_amount: Balance,
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
                    dest_token_address,
                    dest_token_amount,
                    dest_receiver_address,
                    request_nonce: self.request_nonce,
                }),
            );

            self.request_nonce += 1;

            Ok(())
        }

        /// Aggregates request votes cast by guardians and mints/burns tokens
        #[ink(message)]
        pub fn receive_request(
            &mut self,
            request_hash: [u8; 32],
            sender: [u8; 32],
            src_token_address: [u8; 32],
            src_token_amount: u128,
            dest_token_address: [u8; 32],
            dest_token_amount: u128,
            dest_receiver_address: [u8; 32],
            request_nonce: u128,
        ) -> Result<(), MembraneError> {
            let caller = self.env().caller();
            self.is_guardian(caller)?;

            if self.processed_requests.contains(request_hash) {
                return Err(MembraneError::RequestAlreadyProcessed);
            }

            let bytes = Self::concat_u8_arrays(vec![
                &sender,
                &src_token_address,
                &src_token_amount.to_le_bytes(),
                &dest_token_address,
                &dest_token_amount.to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.to_le_bytes(),
            ]);

            let hash = Self::keccak256(&bytes);

            if !request_hash.eq(&hash) {
                return Err(MembraneError::HashDoesNotMatchData);
            }

            match self.pending_requests.get(request_hash) {
                None => {
                    self.pending_requests.insert(
                        request_hash,
                        &Request {
                            dest_token_address,
                            dest_token_amount,
                            dest_receiver_address,
                            signature_count: 0,
                        },
                    );
                }
                Some(mut request) => {
                    self.signatures.insert((request_hash, caller), &());
                    request.signature_count += 1;

                    Self::emit_event(
                        self.env(),
                        Event::SignatureTallied(SignatureTallied {
                            signer: caller,
                            request_hash,
                        }),
                    );

                    if request.signature_count >= self.signature_threshold {
                        self.processed_requests.insert(request_hash, &());
                        self.signatures.remove((request_hash, caller));
                        self.pending_requests.remove(request_hash);

                        self.mint_to(
                            dest_token_address.into(),
                            dest_receiver_address.into(),
                            dest_token_amount,
                        )?;
                    }

                    self.pending_requests.insert(request_hash, &request);

                    Self::emit_event(
                        self.env(),
                        Event::RequestProcessed(RequestProcessed { request_hash }),
                    );
                }
            }

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

        fn concat_u8_arrays(arrays: Vec<&[u8]>) -> Vec<u8> {
            let mut result = Vec::new();
            for array in arrays {
                result.extend_from_slice(array);
            }
            result
        }

        pub fn keccak256(input: &[u8]) -> [u8; 32] {
            let mut output = <Keccak256 as HashOutput>::Type::default();
            hash_bytes::<Keccak256>(input, &mut output);
            output
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
