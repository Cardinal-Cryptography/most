#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::{
    env::{DefaultEnvironment, Environment, Error as InkEnvError},
    prelude::{format, string::String},
};
use scale::{Decode, Encode};

type AccountId = <DefaultEnvironment as Environment>::AccountId;

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum AdvisoryError {
    /// Error when calling a function from contracts environment
    InkEnvError(String),
    /// Caller is not an owner
    NotOwner(AccountId),
}

impl From<InkEnvError> for AdvisoryError {
    fn from(why: InkEnvError) -> Self {
        Self::InkEnvError(format!("{:?}", why))
    }
}

#[ink::trait_definition]
pub trait IsAdvisory {
    #[ink(message)]
    fn set_emergency(&mut self) -> Result<(), AdvisoryError>;
    #[ink(message)]
    fn is_emergency(&self) -> bool;
    #[ink(message)]
    fn set_owner(&mut self, new_owner: AccountId) -> Result<(), AdvisoryError>;
    #[ink(message)]
    fn owner(&self) -> AccountId;
}
