#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod governance {
    use ink::{
        codegen::EmitEvent,
        env::{
            call::{build_call, ExecutionInput},
            hash::{HashOutput, Keccak256},
            hash_bytes, set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec, vec::Vec},
        reflect::ContractEventBase,
        storage::Mapping,
    };
    use scale::{Decode, Encode};
    use shared::Selector;

    #[ink(event)]
    #[derive(Debug)]
    pub struct Fu {
        bar: [u8; 32],
    }

    #[derive(Debug, Encode, Decode, Clone, Copy, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Proposal {
        // signature_count: u128,
        destination: AccountId,
        selector: Selector,
    }

    #[ink(storage)]
    pub struct Governance {
        members: Mapping<AccountId, ()>,
        quorum: u128,
    }

    pub type Event = <Governance as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum GovernanceError {
        Fu,
    }

    // impl From<InkEnvError> for MembraneError {
    //     fn from(why: InkEnvError) -> Self {
    //         Self::InkEnvError(format!("{:?}", why))
    //     }
    // }

    // impl From<PSP22Error> for MembraneError {
    //     fn from(inner: PSP22Error) -> Self {
    //         MembraneError::PSP22(inner)
    //     }
    // }

    impl Governance {
        #[ink(constructor)]
        pub fn new() -> Self {
            todo!("")
        }

        #[ink(message)]
        pub fn submit_proposal(&mut self) -> Result<(), GovernanceError> {
            todo!("")
        }

        #[ink(message)]
        pub fn vote(&mut self) -> Result<(), GovernanceError> {
            todo!("")
        }

        #[ink(message)]
        pub fn execute_proposal(&mut self) -> Result<(), GovernanceError> {
            todo!("")
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
