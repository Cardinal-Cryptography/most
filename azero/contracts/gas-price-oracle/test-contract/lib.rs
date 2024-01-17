#![cfg_attr(not(feature = "std"), no_std, no_main)]

pub use self::test_oracle::TestOracleRef;

#[ink::contract]
pub mod test_oracle {
    use gas_oracle_trait::EthGasPriceOracle;

    #[ink(storage)]
    pub struct TestOracle {
        /// Default price that will be returned
        default_price: u128,
        /// Whether to randomize the price based on block number and timestamp
        randomize: bool,
    }

    impl TestOracle {
        #[ink(constructor)]
        pub fn new(default_price: u128, randomize: bool) -> Self {
            Self {
                default_price,
                randomize,
            }
        }
    }

    impl EthGasPriceOracle for TestOracle {
        #[ink(message)]
        fn get_price(&self) -> (u128, u64) {
            let timestamp = self.env().block_timestamp();
            let price = if self.randomize {
                let block_number = self.env().block_number();
                let mut price = self.default_price;
                let mut randomness = (block_number as u128)
                    .saturating_mul(81111537047593654u128)
                    .saturating_add((timestamp as u128).saturating_mul(19273847364721u128));
                while randomness > 0 {
                    match randomness % 4 {
                        0 => price = price.saturating_mul(10) / 9,
                        1 => price = price.saturating_mul(9) / 10,
                        2 => price = price.saturating_add(randomness % 100000),
                        _ => price = price.saturating_sub(randomness % 100000),
                    }
                    randomness /= 4;
                }
                price
            } else {
                self.default_price
            };
            (price, timestamp)
        }
    }
}
