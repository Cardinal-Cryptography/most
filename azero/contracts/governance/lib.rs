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
    use shared::{
        concat_u8_arrays, keccak256, CallInput, Keccak256HashOutput as HashedProposal, Selector,
    };

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
        /// unique nonce
        nonce: u128,
        /// proposal can be executed only after block with this number
        execute_after: Option<BlockNumber>,
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
        /// number of blocks that has to elapse before a voted in proposal can be executed
        vacation_legis: BlockNumber,
        next_nonce: u128,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum GovernanceError {
        InkEnvError(String),
        ExecuteProposalFailed,
        NotMember,
        ProposalAlreadyProcessed,
        DuplicateProposal,
        Arithmetic,
        NonExistingProposal,
        TimelockedProposal,
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

        /// submits & casts a vote on a proposal
        ///
        /// Can only be called by a member of the governing comittee        
        #[ink(message)]
        pub fn submit_proposal(
            &mut self,
            destination: AccountId,
            selector: Selector,
            args: Vec<u8>,
        ) -> Result<(), GovernanceError> {
            let caller = self.env().caller();
            self.is_guardian(caller)?;

            let nonce = self.next_nonce;

            let bytes = concat_u8_arrays(vec![
                destination.as_ref(),
                &selector,
                &args,
                &nonce.to_le_bytes(),
            ]);
            let hash = keccak256(&bytes);

            if self.processed_proposals.contains(hash) {
                return Err(GovernanceError::ProposalAlreadyProcessed);
            }

            if self.pending_proposals.contains(hash) {
                return Err(GovernanceError::DuplicateProposal);
            }

            self.pending_proposals.insert(
                hash,
                &Proposal {
                    signature_count: 1,
                    destination,
                    selector,
                    args,
                    nonce,
                    execute_after: None,
                },
            );

            self.signatures.insert((hash, caller), &());

            self.next_nonce = nonce.checked_add(1).ok_or(GovernanceError::Arithmetic)?;

            Ok(())
        }

        /// Cast a vote for a transaction
        ///
        /// Can only be called by a member of the governing comittee
        #[ink(message)]
        pub fn vote(&mut self) -> Result<(), GovernanceError> {
            todo!("")
        }

        #[ink(message)]
        pub fn execute_proposal(
            &mut self,
            proposal_hash: HashedProposal,
        ) -> Result<Vec<u8>, GovernanceError> {
            let proposal = self
                .pending_proposals
                .get(proposal_hash)
                .ok_or(GovernanceError::NonExistingProposal)?;

            if proposal.execute_after.expect("must be some") < self.env().block_number() {
                return Err(GovernanceError::TimelockedProposal);
            }

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
            match self.members.contains(account) {
                true => Ok(()),
                false => Err(GovernanceError::NotMember),
            }
        }
    }
}
