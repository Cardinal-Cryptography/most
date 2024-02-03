#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::primitives::AccountId;

#[ink::trait_definition]
pub trait MoneyBox {
    /// Provides a subsidy to the given account.
    #[ink(message)]
    fn pay_out(&self, to: AccountId);
}
