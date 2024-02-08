#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
pub mod oracle {
    use gas_oracle_trait::EthGasPriceOracle;
    use ink::{
        env::{set_code_hash, Error as InkEnvError},
        prelude::{format, string::String},
        storage::{traits::ManualKey, Lazy},
    };
    use ownable::*;
    use scale::{Decode, Encode};

    #[ink(storage)]
    pub struct Oracle {
        /// data for Ownable2Step - oracle owner
        ownable_data: Lazy<ownable::Data, ManualKey<0xDEADBEEF>>,
        /// timestamp of the last update in ms since UNIX epoch
        last_update: u64,
        /// price of one unit of ETH gas in picoAZERO (i.e. 10^-12 AZERO)
        last_price: u128,
        /// Useful for upgrading the contract
        reserved: Option<()>,
    }

    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum OracleError {
        Ownable(ownable::Error),
        InkEnvError(String),
    }

    impl From<InkEnvError> for OracleError {
        fn from(why: InkEnvError) -> Self {
            Self::InkEnvError(format!("{:?}", why))
        }
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct PriceUpdated {
        pub price: u128,
        pub timestamp: u64,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct TransferOwnershipInitiated {
        pub new_owner: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct TransferOwnershipAccepted {
        pub new_owner: AccountId,
    }

    #[ink(event)]
    #[derive(Debug)]
    #[cfg_attr(feature = "std", derive(Eq, PartialEq))]
    pub struct ContractUpgraded {
        pub new_code_hash: [u8; 32],
    }

    impl From<ownable::Error> for OracleError {
        fn from(inner: ownable::Error) -> Self {
            OracleError::Ownable(inner)
        }
    }

    impl Oracle {
        #[ink(constructor)]
        pub fn new(owner: AccountId, init_price: u128) -> Self {
            let mut ownable_data = Lazy::new();
            ownable_data.set(&ownable::Data::new(owner));
            Self {
                ownable_data,
                last_update: Self::env().block_timestamp(),
                last_price: init_price,
                reserved: None,
            }
        }

        #[ink(message)]
        pub fn update_price(&mut self, new_price: u128) -> Result<(), OracleError> {
            self.ensure_owner()?;
            self.last_update = self.env().block_timestamp();
            self.last_price = new_price;
            self.env().emit_event(PriceUpdated {
                price: new_price,
                timestamp: self.last_update,
            });
            Ok(())
        }

        #[ink(message)]
        pub fn upgrade_contract(&mut self, code_hash: [u8; 32]) -> Result<(), OracleError> {
            self.ensure_owner()?;
            set_code_hash(&code_hash)?;
            self.env().emit_event(ContractUpgraded {
                new_code_hash: code_hash,
            });
            Ok(())
        }

        fn ownable_data(&self) -> Result<ownable::Data, ownable::Error> {
            self.ownable_data
                .get()
                .ok_or(ownable::Error::CorruptedStorage)
        }
    }

    impl EthGasPriceOracle for Oracle {
        #[ink(message)]
        /// Returns (price, timestamp) where:
        /// - price is the price of one unit of ETH gas in picoAZERO (i.e. 10^-12 AZERO)
        /// - timestamp is the timestamp of the last update in milliseconds from UNIX epoch
        fn get_price(&self) -> (u128, u64) {
            (self.last_price, self.last_update)
        }
    }

    impl ownable::Ownable2Step for Oracle {
        #[ink(message)]
        fn get_owner(&self) -> OwnableResult<AccountId> {
            Ok(self.ownable_data()?.get_owner())
        }

        #[ink(message)]
        fn get_pending_owner(&self) -> OwnableResult<AccountId> {
            self.ownable_data()?
                .get_pending_owner()
                .ok_or(ownable::Error::NoPendingOwner)
        }

        #[ink(message)]
        fn is_owner(&self, account: AccountId) -> OwnableResult<bool> {
            Ok(self.ownable_data()?.is_owner(account))
        }

        #[ink(message)]
        fn transfer_ownership(&mut self, new_owner: AccountId) -> OwnableResult<()> {
            let mut ownable_data = self.ownable_data()?;
            ownable_data.transfer_ownership(self.env().caller(), new_owner)?;
            self.ownable_data.set(&ownable_data);
            self.env()
                .emit_event(TransferOwnershipInitiated { new_owner });
            Ok(())
        }

        #[ink(message)]
        fn accept_ownership(&mut self) -> OwnableResult<()> {
            let new_owner = self.env().caller();
            let mut ownable_data = self.ownable_data()?;
            ownable_data.accept_ownership(new_owner)?;
            self.ownable_data.set(&ownable_data);
            self.env()
                .emit_event(TransferOwnershipAccepted { new_owner });
            Ok(())
        }

        #[ink(message)]
        fn ensure_owner(&self) -> OwnableResult<()> {
            self.ownable_data()?.ensure_owner(self.env().caller())
        }
    }

    #[cfg(test)]
    mod tests {
        use ink::env::{
            test::{default_accounts, set_caller},
            DefaultEnvironment,
        };

        use super::*;
        type DefEnv = DefaultEnvironment;

        #[ink::test]
        fn get_price_works_after_initialization() {
            let owner = default_accounts::<DefEnv>().alice;
            let init_price = 100;
            set_caller::<DefEnv>(owner);
            let oracle = Oracle::new(owner, init_price);
            assert_eq!(oracle.get_price(), (init_price, 0));
        }

        #[ink::test]
        fn get_price_works_after_update() {
            let owner = default_accounts::<DefEnv>().alice;
            let init_price = 100;
            set_caller::<DefEnv>(owner);
            let mut oracle = Oracle::new(owner, init_price);
            let new_price = 200;
            assert!(init_price != new_price);
            oracle.update_price(new_price).unwrap();
            assert_eq!(oracle.get_price(), (new_price, 0));
        }

        #[ink::test]
        fn get_price_fails_on_non_owner() {
            let owner = default_accounts::<DefEnv>().alice;
            let non_owner = default_accounts::<DefEnv>().bob;
            let init_price = 100;
            set_caller::<DefEnv>(owner);
            let mut oracle = Oracle::new(owner, init_price);
            let new_price = 200;
            set_caller::<DefEnv>(non_owner);
            assert!(oracle.update_price(new_price).is_err());
            assert_eq!(oracle.get_price(), (init_price, 0));
        }

        #[ink::test]
        fn set_owner_works() {
            let owner = default_accounts::<DefEnv>().alice;
            let new_owner = default_accounts::<DefEnv>().bob;
            let init_price = 100;
            set_caller::<DefEnv>(owner);
            let mut oracle = Oracle::new(owner, init_price);
            let new_price = 200;
            set_caller::<DefEnv>(new_owner);
            assert!(oracle.update_price(new_price).is_err());
            set_caller::<DefEnv>(owner);
            oracle.transfer_ownership(new_owner);
            // before `new owner` accepts ownership, the old `owner` holds the role
            assert!(oracle.update_price(new_price).is_ok());
            set_caller::<DefEnv>(new_owner);
            assert!(oracle.update_price(new_price).is_err());
            oracle.accept_ownership();
            // below Alice is not the owner anymore
            set_caller::<DefEnv>(owner);
            assert!(oracle.update_price(new_price).is_err());

            set_caller::<DefEnv>(new_owner);
            assert!(oracle.update_price(new_price).is_ok());
            assert_eq!(oracle.get_price(), (new_price, 0));
        }
    }
}
