#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
pub mod token {
    use scale::{Decode, Encode};
    use ink::prelude::string::String;

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct PocketMoneyPaidOut {
        #[ink(topic)]
        pub to: AccountId,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MoneyBoxError {
        CallerNotAdmin,
        CallerNotOwner,
        NotEnoughBalance,
        InkEnvError(String),
    }

    #[ink(storage)]
    pub struct MoneyBox {
        amount_to_pay: Balance,
        owner: AccountId,
        admin: AccountId,
    }

    impl MoneyBox {
        #[ink(constructor)]
        pub fn new(
            amount_to_pay: Balance,
            owner: AccountId,
            admin: AccountId,
        ) -> Self {
            Self {
                amount_to_pay,
                owner,
                admin,
            }
        }

        #[ink(message)]
        pub fn pay_out(&mut self, to: AccountId) -> Result<(), MoneyBoxError> {
            self.ensure_owner()?;
            if self.env().balance() < self.amount_to_pay {
                return Err(MoneyBoxError::NotEnoughBalance);
            }

            self.env().transfer(to, self.amount_to_pay).map_err(|e| {
                MoneyBoxError::InkEnvError(format!("Failed to transfer: {:?}", e))
            })?;
            self.env().emit_event(PocketMoneyPaidOut { to });
            Ok(())
        }

        #[ink(message)]
        pub fn owner(&self) -> AccountId {
            self.owner
        }

        #[ink(message)]
        pub fn set_owner(&mut self, new_owner: AccountId) -> Result<(), MoneyBoxError> {
            self.ensure_admin()?;
            self.owner = new_owner;
            Ok(())
        }

        #[ink(message)]
        pub fn admin(&self) -> AccountId {
            self.admin
        }

        #[ink(message)]
        pub fn set_admin(&mut self, new_admin: AccountId) -> Result<(), MoneyBoxError> {
            self.ensure_admin()?;
            self.admin = new_admin;
            Ok(())
        }

        #[ink(message)]
        pub fn amount_to_pay(&self) -> Balance {
            self.amount_to_pay
        }

        #[ink(message)]
        pub fn set_amount_to_pay(&mut self, new_amount: Balance) -> Result<(), MoneyBoxError> {
            self.ensure_admin()?;
            self.amount_to_pay = new_amount;
            Ok(())
        }

        fn ensure_owner(&self) -> Result<(), MoneyBoxError> {
            if self.env().caller() != self.admin {
                return Err(MoneyBoxError::CallerNotOwner);
            } else {
                Ok(())
            }
        }

        fn ensure_admin(&self) -> Result<(), MoneyBoxError> {
            if self.env().caller() != self.admin {
                return Err(MoneyBoxError::CallerNotAdmin);
            } else {
                Ok(())
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use ink::env::{test::*, DefaultEnvironment as E};

        use super::*;

        const DEFAULT_AZERO_AMOUNT: Balance = 1000000000;
        const NEW_AZERO_AMOUNT: Balance = 2000000000;

        #[ink::test]
        fn set_admin_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            let mut token = MoneyBox::new(DEFAULT_AZERO_AMOUNT, alice, bob);

            set_caller::<E>(alice);
            assert_eq!(token.set_admin(alice), Err(MoneyBoxError::CallerNotAdmin));

            set_caller::<E>(bob);
            assert_eq!(token.set_admin(alice), Ok(()));
            assert_eq!(token.admin(), alice);
        }

        #[ink::test]
        fn set_owner_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            let mut token = MoneyBox::new(DEFAULT_AZERO_AMOUNT, alice, bob);

            set_caller::<E>(alice);
            assert_eq!(token.set_owner(alice), Err(MoneyBoxError::CallerNotAdmin));

            set_caller::<E>(bob);
            assert_eq!(token.set_owner(alice), Ok(()));
            assert_eq!(token.owner(), alice);
        }

        #[ink::test]
        fn set_amount_to_pay_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            let mut token = MoneyBox::new(DEFAULT_AZERO_AMOUNT, alice, bob);

            set_caller::<E>(alice);
            assert_eq!(token.set_amount_to_pay(NEW_AZERO_AMOUNT), Err(MoneyBoxError::CallerNotAdmin));

            set_caller::<E>(bob);
            assert_eq!(token.set_amount_to_pay(NEW_AZERO_AMOUNT), Ok(()));
            assert_eq!(token.amount_to_pay(), NEW_AZERO_AMOUNT);
        }

        #[ink::test]
        fn pay_out_fails_when_not_enough_funds() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            let mut token = MoneyBox::new(DEFAULT_AZERO_AMOUNT, alice, bob);

            set_caller::<E>(alice);
            assert_eq!(token.pay_out(bob), Err(MoneyBoxError::CallerNotOwner));

            set_caller::<E>(bob);
            assert_eq!(token.pay_out(bob), Err(MoneyBoxError::NotEnoughBalance));

            //let bob_balance_after = get_account_balance::<E>(bob).expect("Cannot get balance");
            //assert_eq!(bob_balance_after, bob_balance_before + DEFAULT_AZERO_AMOUNT);
        }

        #[ink::test]
        fn pay_out_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            let mut token = MoneyBox::new(DEFAULT_AZERO_AMOUNT, alice, bob);

            // transfer some funds to the contract
            set_account_balance::<E>(alice, 2 * DEFAULT_AZERO_AMOUNT);
            set_caller::<E>(alice);
            transfer_in::<E>(DEFAULT_AZERO_AMOUNT);

            let bob_balance_before = get_account_balance::<E>(bob).expect("Cannot get balance");

            set_caller::<E>(alice);
            assert_eq!(token.pay_out(bob), Err(MoneyBoxError::CallerNotOwner));

            set_caller::<E>(bob);
            assert_eq!(token.pay_out(bob), Ok(()));

            let bob_balance_after = get_account_balance::<E>(bob).expect("Cannot get balance");
            assert_eq!(bob_balance_after, bob_balance_before + DEFAULT_AZERO_AMOUNT);
        }
    }
}
