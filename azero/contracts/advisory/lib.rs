#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
pub mod advisory {

    use ink::{
        env::Error as InkEnvError,
        prelude::{format, string::String},
    };
    use scale::{Decode, Encode};

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

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct EmergencyChanged {
        pub emergency: bool,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct OwnershipTransferred {
        pub previous_owner: AccountId,
        pub new_owner: AccountId,
    }

    #[ink(storage)]
    pub struct Advisory {
        pub owner: AccountId,
        pub emergency: bool,
    }

    impl Advisory {
        #[ink(message)]
        pub fn flip_emergency(&mut self) -> Result<(), AdvisoryError> {
            self.ensure_owner()?;
            self.emergency = !self.emergency;
            self.env().emit_event(EmergencyChanged {
                emergency: self.emergency,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn is_emergency(&self) -> bool {
            self.emergency
        }

        #[ink(message)]
        pub fn transfer_ownership(&mut self, new_owner: AccountId) -> Result<(), AdvisoryError> {
            self.ensure_owner()?;
            self.env().emit_event(OwnershipTransferred {
                previous_owner: self.owner,
                new_owner,
            });
            self.owner = new_owner;
            Ok(())
        }

        #[ink(message)]
        pub fn owner(&self) -> AccountId {
            self.owner
        }
    }

    impl Advisory {
        #[ink(constructor)]
        pub fn new(owner: AccountId) -> Self {
            Self {
                emergency: false,
                owner,
            }
        }

        fn ensure_owner(&mut self) -> Result<(), AdvisoryError> {
            let caller = self.env().caller();
            match caller.eq(&self.owner) {
                true => Ok(()),
                false => Err(AdvisoryError::NotOwner(caller)),
            }
        }
    }
}
