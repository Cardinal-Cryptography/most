#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod migrations {

    use ink::{
        env::Error as InkEnvError,
        prelude::{format, string::String},
    };
    use scale::{Decode, Encode};

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum MigrationsError {
        InkEnvError(String),
    }

    impl From<InkEnvError> for MigrationsError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    #[ink(storage)]
    pub struct Migrations {
        last_completed_migration: u32,
        owner: AccountId,
    }

    impl Migrations {
        #[ink(constructor)]
        pub fn new(owner: AccountId) -> Self {
            Self {
                last_completed_migration: 0,
                owner,
            }
        }

        #[ink(message)]
        pub fn set_completed(&mut self, completed: u32) {
            self.last_completed_migration = completed;
        }

        #[ink(message)]
        pub fn code_hash(&self) -> Result<Hash, MigrationsError> {
            Ok(self.env().own_code_hash()?)
        }

        #[ink(message)]
        pub fn last_completed_migration(&self) -> u32 {
            self.last_completed_migration
        }
    }
}
