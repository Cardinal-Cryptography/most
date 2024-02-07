#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::primitives::AccountId;
use scale::{Decode, Encode};

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum Error {
    UnauthorizedAccount(AccountId),
    NoPendingOwner,
    CorruptedStorage,
}

pub type OwnableResult<T> = Result<T, Error>;

#[derive(Debug)]
#[ink::storage_item]
pub struct Data {
    owner: AccountId,
    pending_owner: Option<AccountId>,
}

impl Data {
    pub fn new(owner: AccountId) -> Self {
        Self { owner, pending_owner: None }
    }

    pub fn transfer_ownership(&mut self, caller: AccountId, new_owner: AccountId) -> OwnableResult<()> {
        if caller != self.owner {
            return Err(Error::UnauthorizedAccount(caller));
        }

        self.pending_owner = Some(new_owner);

        Ok(())
    }

    pub fn accept_ownership(&mut self, caller: AccountId) -> OwnableResult<()> {
        let pending_owner = self.pending_owner.ok_or(Error::NoPendingOwner)?;

        if caller != pending_owner {
            return Err(Error::UnauthorizedAccount(caller));
        }

        self.owner = pending_owner;
        self.pending_owner = None;

        Ok(())
    }

    pub fn get_owner(&self) -> AccountId {
        self.owner
    }

    pub fn get_pending_owner(&self) -> Option<AccountId> {
        self.pending_owner
    }

    pub fn is_owner(&self, caller: AccountId) -> bool {
        caller == self.owner
    }

    pub fn ensure_owner(&self, caller: AccountId) -> OwnableResult<()> {
        if caller != self.owner {
            Err(Error::UnauthorizedAccount(caller))
        } else {
            Ok(())
        }
    }

}


/// Implement this trait to enable two-step ownership trasfer process in your contract.
/// 
/// The process looks like this:
/// * current owner (Alice) calls `self.transfer_ownership(bob)`,
/// * the contract still has the owner: Alice and a pending owner: bob,
/// * when Bob claims the ownership by calling `self.accept_ownership()` he becomes the new owner and pending owner is removed.
/// 
/// The methods are all wrapper in `OwnableResult` to make it possible to use them in settings where the `Data` is e.g. behid `Lazy`.
#[ink::trait_definition]
pub trait Ownable2Step {

    #[ink(message)]
    fn get_owner(&self) -> OwnableResult<AccountId>;

    #[ink(message)]
    fn get_pending_owner(&self) -> OwnableResult<AccountId>;

    #[ink(message)]
    fn is_owner(&self, account: AccountId) -> OwnableResult<bool>;

    #[ink(message)]
    fn transfer_ownership(&mut self, new_owner: AccountId) -> OwnableResult<()>;

    #[ink(message)]
    fn accept_ownership(&mut self) -> OwnableResult<()>;

    #[ink(message)]
    fn ensure_owner(&self) -> OwnableResult<()>;
}
