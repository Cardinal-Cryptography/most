#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum OwnableError {
    UnauthorizedAccount(AccountId),
    NoPendingOwner,
}

pub type OwnableResult<T> = Result<T, OwnableError>;

pub struct OwnableData {
    owner: AccountId,
    pending_owner: Option<AccountId>,
}

impl OwnableData {
    pub fn new(owner: AccountId) -> Self {
        Self { owner, pending_owner: None }
    }

    pub fn transfer_ownership(&mut self, caller: AccountId, new_owner: AccountId) -> OwnableResult<()> {
        if caller != self.owner {
            return Err(OwnershipError::UnauthorizedAccount(caller));
        }

        self.pending_owner = Some(new_owner);

        Ok(())
    }

    pub fn accept_ownership(&mut self, caller: AccountId) -> OwnableResult<()> {
        if Some(caller) != self.pending_owner {
            return Err(OwnershipError::UnauthorizedAccount(caller));
        }

        if let Some(pending_owner) = self.pending_owner {
            self.owner = pending_owner;
            self.pending_owner = None;
        } else {
            return Err(OwnershipError::NoPendingOwner);
        }

        Ok(())
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner
    }

    pub fn is_owner(&self, caller: AccountId) -> bool {
        caller == self.owner
    }
}

#[ink::trait_definition]
trait Ownable2Step {

    fn get_data(&self) -> OwnableData;

    fn get_owner(&self) {
        self.get_data().get_owner();
    }
}