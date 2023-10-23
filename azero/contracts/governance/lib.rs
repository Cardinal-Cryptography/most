#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod governance {
    use ink::{
        codegen::EmitEvent,
        env::{
            call::{build_call, utils::ArgumentList, ExecutionInput},
            hash::{HashOutput, Keccak256},
            hash_bytes, set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec, vec::Vec},
        reflect::ContractEventBase,
        storage::Mapping,
    };
    use scale::{Decode, Encode};
    use shared::{CallInput, Selector};

    #[ink(event)]
    #[derive(Debug)]
    pub struct ProposalExecuted {
        hash: [u8; 32],
        result: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Proposal {
        signature_count: u64,
        destination: AccountId,
        selector: Selector,
        args: Vec<u8>,
    }

    #[ink(storage)]
    pub struct Governance
    where
        T: scale::Encode,
    {
        members: Mapping<AccountId, ()>,
        quorum: u128,
        pending_proposals: Mapping<[u8; 32], Proposal>,
    }

    // pub type Event = <Governance as ContractEventBase>::Type;

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum GovernanceError {
        InkEnvError(String),
        ExecuteProposalFailed,
    }

    impl From<InkEnvError> for GovernanceError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
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
        pub fn execute_proposal(
            &mut self,
            proposal_hash: [u8; 32],
        ) -> Result<Vec<u8>, GovernanceError> {
            // TOOD : timelock

            let proposal = self.pending_proposals.get(proposal_hash).unwrap();

            match build_call::<<Self as ::ink::env::ContractEnv>::Env>()
                .call(proposal.destination)
                .exec_input(
                    ExecutionInput::new(ink::env::call::Selector::new(proposal.selector))
                        .push_arg(CallInput(&proposal.args)),
                )
                .returns::<Vec<u8>>()
                .try_invoke()
            {
                Ok(Ok(result)) => {
                    self.env().emit_event(ProposalExecuted {
                        hash: proposal_hash,
                        result: result.clone(),
                    });

                    Ok(result)
                }
                _ => Err(GovernanceError::ExecuteProposalFailed),
            }
        }
    }
}
