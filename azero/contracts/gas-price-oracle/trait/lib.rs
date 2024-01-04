#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::trait_definition]
pub trait EthGasPriceOracle {
    #[ink(message)]
    /// Returns (price, timestamp), where:
    /// - price is the price of one unit of ETH gas in picoAZERO (i.e. 10^-12 AZERO)
    /// - timestamp is the timestamp of the last update in milliseconds since the UNIX epoch
    fn get_price(&self) -> (u128, u64);
}
