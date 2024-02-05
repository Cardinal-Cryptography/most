#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
pub mod advisory {

    use advisory_trait::{AdvisoryError, IsAdvisory};

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

    impl IsAdvisory for Advisory {
        #[ink(message)]
        fn flip_emergency(&mut self) -> Result<(), AdvisoryError> {
            self.ensure_owner()?;
            self.emergency = !self.emergency;
            self.env().emit_event(EmergencyChanged {
                emergency: self.emergency,
            });
            Ok(())
        }

        #[ink(message)]
        fn is_emergency(&self) -> bool {
            self.emergency
        }

        #[ink(message)]
        fn transfer_ownership(&mut self, new_owner: AccountId) -> Result<(), AdvisoryError> {
            self.ensure_owner()?;
            self.env().emit_event(OwnershipTransferred {
                previous_owner: self.owner,
                new_owner,
            });
            self.owner = new_owner;
            Ok(())
        }

        #[ink(message)]
        fn owner(&self) -> AccountId {
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
