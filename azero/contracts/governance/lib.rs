#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod governance {
    use ink::{
        env::{
            call::{build_call, utils::ArgumentList, ExecutionInput},
            hash::{HashOutput, Keccak256},
            hash_bytes, set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec, vec::Vec},
        storage::Mapping,
    };
    use scale::{Decode, Encode};
    use shared::{CallInput, Keccak256HashOutput as HashedProposal, Selector};

    #[ink(event)]
    #[derive(Debug)]
    pub struct ProposalExecuted {
        hash: HashedProposal,
        result: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Proposal {
        signature_count: u64,
        /// The address of the contract
        destination: AccountId,
        /// The selector bytes that identify the contracts function        
        selector: Selector,
        /// The SCALE encoded arguments of the contracts function.        
        args: Vec<u8>,
    }

    #[ink(storage)]
    pub struct Governance {
        /// whitelised accounts that can propose & vote on proposals
        members: Mapping<AccountId, ()>,
        /// Minimum number of members that have to confirm a proposal before it can be executed.
        quorum: u128,
        pending_proposals: Mapping<HashedProposal, Proposal>,
        signatures: Mapping<(HashedProposal, AccountId), ()>,
        processed_proposals: Mapping<HashedProposal, ()>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum GovernanceError {
        InkEnvError(String),
        ExecuteProposalFailed,
        NotMember,
    }

    impl From<InkEnvError> for GovernanceError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    impl Governance {
        #[ink(constructor)]
        pub fn new() -> Self {
            todo!("")
        }

        #[ink(message)]
        pub fn submit_proposal(&mut self, proposal: Proposal) -> Result<(), GovernanceError> {
            let caller = self.env().caller();
            self.is_guardian(caller)?;

            // self.pending_proposals.insert(key, value)

            todo!("")
        }

        #[ink(message)]
        pub fn vote(&mut self) -> Result<(), GovernanceError> {
            todo!("")
        }

        #[ink(message)]
        pub fn execute_proposal(
            &mut self,
            proposal_hash: HashedProposal,
        ) -> Result<Vec<u8>, GovernanceError> {
            // TODO : timelock

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

        fn is_guardian(&self, account: AccountId) -> Result<(), GovernanceError> {
            match self.members.contains(&account) {
                true => Ok(()),
                false => Err(GovernanceError::NotMember),
            }
        }
    }
}
