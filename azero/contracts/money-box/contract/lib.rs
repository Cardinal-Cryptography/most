#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::money_box_contract::MoneyBoxContractRef;

#[ink::contract]
pub mod money_box_contract {
    use ink::{
        env::{set_code_hash, Error as InkEnvError},
        prelude::{format, string::String},
    };
    use money_box_trait::MoneyBox;
    use scale::{Decode, Encode};

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct PocketMoneyPaidOut {
        #[ink(topic)]
        pub to: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct InsufficientFunds {
        pub current_balance: Balance,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct ContractUpgraded {
        pub new_code_hash: [u8; 32],
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MoneyBoxError {
        CallerNotAdmin,
        InkEnvError(String),
    }

    impl From<InkEnvError> for MoneyBoxError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    #[ink(storage)]
    pub struct MoneyBoxContract {
        amount_to_pay: Balance,
        owner: AccountId,
        admin: AccountId,
    }

    impl MoneyBoxContract {
        #[ink(constructor)]
        pub fn new(amount_to_pay: Balance) -> Self {
            Self {
                amount_to_pay,
                owner: Self::env().caller(),
                admin: Self::env().caller(),
            }
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

        #[ink(message)]
        pub fn upgrade_contract(&mut self, code_hash: [u8; 32]) -> Result<(), MoneyBoxError> {
            self.ensure_admin()?;
            set_code_hash(&code_hash)?;
            self.env().emit_event(ContractUpgraded {
                new_code_hash: code_hash,
            });
            Ok(())
        }

        fn ensure_admin(&self) -> Result<(), MoneyBoxError> {
            if self.env().caller() != self.admin {
                Err(MoneyBoxError::CallerNotAdmin)
            } else {
                Ok(())
            }
        }
    }

    impl MoneyBox for MoneyBoxContract {
        #[ink(message)]
        fn pay_out(&self, to: AccountId) {
            if self.env().balance() < self.amount_to_pay {
                self.env().emit_event(InsufficientFunds {
                    current_balance: self.env().balance(),
                });
            } else if self.env().caller() == self.owner
                && self.env().transfer(to, self.amount_to_pay).is_ok()
            {
                self.env().emit_event(PocketMoneyPaidOut { to });
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

            set_caller::<E>(alice);
            let mut contract = MoneyBoxContract::new(DEFAULT_AZERO_AMOUNT);

            set_caller::<E>(bob);
            assert_eq!(contract.set_admin(bob), Err(MoneyBoxError::CallerNotAdmin));

            set_caller::<E>(alice);
            assert_eq!(contract.set_admin(bob), Ok(()));
            assert_eq!(contract.admin(), bob);
        }

        #[ink::test]
        fn set_owner_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            set_caller::<E>(alice);
            let mut contract = MoneyBoxContract::new(DEFAULT_AZERO_AMOUNT);

            set_caller::<E>(bob);
            assert_eq!(contract.set_owner(bob), Err(MoneyBoxError::CallerNotAdmin));

            set_caller::<E>(alice);
            assert_eq!(contract.set_owner(bob), Ok(()));
            assert_eq!(contract.owner(), bob);
        }

        #[ink::test]
        fn set_amount_to_pay_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            set_caller::<E>(alice);
            let mut contract = MoneyBoxContract::new(DEFAULT_AZERO_AMOUNT);

            set_caller::<E>(bob);
            assert_eq!(
                contract.set_amount_to_pay(NEW_AZERO_AMOUNT),
                Err(MoneyBoxError::CallerNotAdmin)
            );

            set_caller::<E>(alice);
            assert_eq!(contract.set_amount_to_pay(NEW_AZERO_AMOUNT), Ok(()));
            assert_eq!(contract.amount_to_pay(), NEW_AZERO_AMOUNT);
        }

        #[ink::test]
        fn pay_out_fails_when_caller_not_owner() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            set_caller::<E>(alice);
            let contract = MoneyBoxContract::new(DEFAULT_AZERO_AMOUNT);

            // transfer some funds to the contract
            set_account_balance::<E>(alice, 2 * DEFAULT_AZERO_AMOUNT);
            transfer_in::<E>(DEFAULT_AZERO_AMOUNT);

            set_caller::<E>(bob);
            contract.pay_out(alice);
            assert_eq!(recorded_events().count(), 0);
        }

        #[ink::test]
        fn pay_out_works() {
            let alice = default_accounts::<E>().alice;
            let bob = default_accounts::<E>().bob;

            set_caller::<E>(alice);
            let contract = MoneyBoxContract::new(DEFAULT_AZERO_AMOUNT);

            // transfer some funds to the contract
            set_account_balance::<E>(alice, 2 * DEFAULT_AZERO_AMOUNT);
            transfer_in::<E>(DEFAULT_AZERO_AMOUNT);

            let bob_balance_before = get_account_balance::<E>(bob).expect("Cannot get balance");

            contract.pay_out(bob);

            let bob_balance_after = get_account_balance::<E>(bob).expect("Cannot get balance");
            assert_eq!(bob_balance_after, bob_balance_before + DEFAULT_AZERO_AMOUNT);
        }
    }
}
