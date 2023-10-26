#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod governance {
    use ink::{
        env::{
            call::{build_call, ExecutionInput},
            set_code_hash, DefaultEnvironment, Error as InkEnvError,
        },
        prelude::{format, string::String, vec::Vec},
        storage::Mapping,
    };
    use scale::{Decode, Encode};
    use shared::{CallInput, Selector};

    type ProposalId = u128;

    #[ink(event)]
    #[derive(Debug)]
    pub struct ProposalSubmitted {
        by: AccountId,
        id: ProposalId,
        proposal: Proposal,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct Vote {
        by: AccountId,
        proposal: ProposalId,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct ProposalExecuted {
        by: AccountId,
        id: ProposalId,
        result: Vec<u8>,
    }

    #[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
    #[cfg_attr(
        feature = "std",
        derive(scale_info::TypeInfo, ink::storage::traits::StorageLayout)
    )]
    pub struct Proposal {
        /// The address of the contract
        destination: AccountId,
        /// The selector bytes that identify the contracts function
        selector: Selector,
        /// The SCALE encoded arguments of the contracts function.
        args: Vec<u8>,
    }

    #[ink(storage)]
    pub struct Governance {
        /// owner, typicaly set to be the governance contract itself
        owner: AccountId,
        /// The whitelised accounts that can propose & vote on proposals
        members: Mapping<AccountId, ()>,
        /// The Minimum number of members that have to confirm a proposal before it can be executed
        quorum: u32,
        /// The set of votes cast by the members of governing comittee
        signatures: Mapping<(ProposalId, AccountId), ()>,
        /// The amount of votes per proposal
        signature_count: Mapping<ProposalId, u32>,
        /// non-executed transactions
        pending_proposals: Mapping<ProposalId, Proposal>,
        /// next id
        next_proposal_id: u128,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum GovernanceError {
        InkEnvError(String),
        ExecuteProposalFailed,
        MemberAccount,
        NotMember,
        Arithmetic,
        NonExistentProposal,
        ProposalAlreadySigned,
        NoQuorum,
        NotOwner,
    }

    impl From<InkEnvError> for GovernanceError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    impl Governance {
        /// Constructs a new instance of the governance contract
        ///
        /// Caller becomes the owner, typically you'd want to set the owner to be the contract itself immediately after instantiating
        #[ink(constructor)]
        pub fn new(quorum: u32) -> Self {
            Self {
                owner: Self::env().caller(),
                members: Mapping::new(),
                quorum,
                signatures: Mapping::new(),
                signature_count: Mapping::new(),
                pending_proposals: Mapping::new(),
                next_proposal_id: 0,
            }
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
            self.ensure_member(caller)?;

            let id = self.next_proposal_id;

            let proposal = Proposal {
                destination,
                selector,
                args,
            };

            self.pending_proposals.insert(id, &proposal);
            self.signatures.insert((id, caller), &());
            self.signature_count.insert(id, &1);

            self.next_proposal_id = id.checked_add(1).ok_or(GovernanceError::Arithmetic)?;

            self.env().emit_event(ProposalSubmitted {
                by: caller,
                id,
                proposal,
            });

            Ok(())
        }

        /// Cast a vote for a transaction
        ///
        /// Can only be called by a member of the governing comittee
        #[ink(message)]
        pub fn vote(&mut self, proposal_id: ProposalId) -> Result<(), GovernanceError> {
            let caller = self.env().caller();
            self.ensure_member(caller)?;

            if self.signatures.contains((proposal_id, caller)) {
                return Err(GovernanceError::ProposalAlreadySigned);
            }

            let count = self
                .signature_count
                .get(proposal_id)
                .ok_or(GovernanceError::NonExistentProposal)?
                .checked_add(1)
                .ok_or(GovernanceError::Arithmetic)?;

            self.signatures.insert((proposal_id, caller), &());
            self.signature_count.insert(proposal_id, &count);

            self.env().emit_event(Vote {
                by: caller,
                proposal: proposal_id,
            });

            Ok(())
        }

        /// Execute a proposal if it has reached a quorum
        ///
        /// Can be called by anyone
        #[ink(message)]
        pub fn execute_proposal(
            &mut self,
            proposal_id: ProposalId,
        ) -> Result<Vec<u8>, GovernanceError> {
            self.ensure_quorum(proposal_id)?;

            let proposal = self
                .pending_proposals
                .get(proposal_id)
                .ok_or(GovernanceError::NonExistentProposal)?;

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
                        by: self.env().caller(),
                        id: proposal_id,
                        result: result.clone(),
                    });

                    // clean up
                    self.pending_proposals.remove(proposal_id);
                    self.signature_count.remove(proposal_id);

                    Ok(result)
                }
                _ => Err(GovernanceError::ExecuteProposalFailed),
            }
        }

        /// Is this account a member of the governing comittee?
        pub fn is_member(&self, account: AccountId) -> bool {
            self.members.contains(account)
        }

        /// Has this proposal reached a quorum yet?
        ///
        /// Returns an error if proposal does not exist
        pub fn has_quorum(&self, proposal_id: ProposalId) -> Result<bool, GovernanceError> {
            if self.get_signature_count(proposal_id)? < self.quorum {
                return Ok(false);
            }
            Ok(true)
        }

        /// Returns a vote count for a given proposal
        ///
        /// Reverts if proposal does not exist
        pub fn get_signature_count(&self, proposal_id: ProposalId) -> Result<u32, GovernanceError> {
            self.signature_count.get(proposal_id).ok_or(GovernanceError::NonExistentProposal)
        }

        /// Adds a member to the governance whitelist
        ///
        /// Can only be called by contracts owner (typically the contract itself)
        pub fn add_member(&mut self, account: AccountId) -> Result<(), GovernanceError> {
            self.ensure_owner()?;
            self.members.insert(account, &());
            Ok(())
        }

        /// Removes a member from governance whitelist
        ///
        /// Can only be called by the contracts owner (typically the contract itself)
        pub fn remove_member(&mut self, account: AccountId) -> Result<(), GovernanceError> {
            self.ensure_owner()?;
            self.members.remove(account);
            Ok(())
        }

        /// Clean up past & ongoing signatures and get back storage deposit in return
        ///
        /// Can be called by anyone but will revert if the account in question is a present member of the governing comittee.
        /// This message is a separate tx and not part of e.g. `remove_member` tx as there are no guarantess it will fit within one block
        pub fn clean_signatures(&mut self, account: AccountId) -> Result<(), GovernanceError> {
            if self.is_member(account) {
                return Err(GovernanceError::MemberAccount);
            }

            (0..self.next_proposal_id).for_each(|id| {
                self.signatures.remove((id, account));
            });

            Ok(())
        }

        /// Sets a new owner account
        ///
        /// Can only be called by the contracts owner (typically the contract itself)        
        pub fn set_owner(&mut self, new_owner: AccountId) -> Result<(), GovernanceError> {
            self.ensure_owner()?;
            self.owner = new_owner;
            Ok(())
        }

        /// Sets a new threshold for quorum
        ///
        /// Can only be called by the contracts owner (typically the contract itself)
        pub fn set_quorum(&mut self, new_quorum: u32) -> Result<(), GovernanceError> {
            self.ensure_owner()?;
            self.quorum = new_quorum;
            Ok(())
        }

        /// Upgrades contract code
        ///
        /// Can only be called by the contracts owner (typically the contract itself)
        #[ink(message)]
        pub fn set_code(
            &mut self,
            code_hash: [u8; 32],
            callback: Option<Selector>,
        ) -> Result<(), GovernanceError> {
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
                    .returns::<Result<(), GovernanceError>>()
                    .invoke()?;
            }

            Ok(())
        }

        fn ensure_member(&self, account: AccountId) -> Result<(), GovernanceError> {
            match self.is_member(account) {
                true => Ok(()),
                false => Err(GovernanceError::NotMember),
            }
        }

        fn ensure_quorum(&self, proposal_id: ProposalId) -> Result<(), GovernanceError> {
            if !self.has_quorum(proposal_id)? {
                return Err(GovernanceError::NoQuorum);
            }
            Ok(())
        }

        fn ensure_owner(&mut self) -> Result<(), GovernanceError> {
            let caller = self.env().caller();
            match caller.eq(&self.owner) {
                true => Ok(()),
                false => Err(GovernanceError::NotOwner),
            }
        }
    }
}
