#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod membrane {
    #[cfg(feature = "std")]
    use ink::storage::traits::StorageLayout;
    use ink::{
        codegen::EmitEvent,
        prelude::{format, string::String, vec, vec::Vec},
        reflect::ContractEventBase,
        storage::{traits::ManualKey, Mapping},
    };
    use scale::{Decode, Encode};

    #[ink(event)]
    #[derive(Debug)]
    pub struct Fu {}

    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo, StorageLayout))]
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
        processed_requests: Mapping<[u8; 32], ()>,
        guardians: Mapping<AccountId, ()>,
    }

    pub type Event = <Membrane as ContractEventBase>::Type;

    impl Membrane {
        #[ink(constructor)]
        pub fn new(guardians: Vec<AccountId>, signature_threshold: u128) -> Self {
            todo!()
        }

        #[ink(message)]
        pub fn flip(&mut self) {
            // Self::emit_event(self.env(), Event::Flip(Flip { value: self.flip }));

            todo!()
        }

        // #[ink(message)]
        // pub fn flop(&mut self) {
        //     self.flop = !self.flop;
        //     Self::emit_event(self.env(), Event::Flop(Flop { value: self.flop }));
        // }

        // #[ink(message)]
        // pub fn get(&self) -> (bool, bool) {
        //     (self.flip, self.flop)
        // }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
