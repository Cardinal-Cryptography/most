use anyhow::{anyhow, Result};
use ethers::types::U256;

/// Struct holding the current balances on both chains for a given account pair.
/// It includes the native coins, and all the wrapped tokens we're concerned with.
/// It also serves as a model for the transfers we want to test, each such transfer
/// being simulated by a method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Balance {
    pub eth: U256,
    pub weth_eth: u128,
    pub wazero_eth: u128,
    pub usdt_eth: u128,
    pub azero: u128,
    pub weth_azero: u128,
    pub wazero_azero: u128,
    pub usdt_azero: u128,
}

impl Balance {
    /// Wrap ETH into wETH.
    pub fn wrap_weth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.eth = balance
            .eth
            .checked_sub(transfer_amount.into())
            .ok_or(anyhow!("Insufficient ETH."))?;
        balance.weth_eth = balance
            .weth_eth
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wETH overflow."))?;
        Ok(balance)
    }

    /// Wrap AZERO into wAZERO.
    pub fn wrap_wazero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.azero = balance
            .azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient AZERO."))?;
        balance.wazero_azero = balance
            .wazero_azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wAZERO overflow."))?;
        Ok(balance)
    }

    /// Bridge ETH to wETH.
    pub fn bridge_eth_eth_to_azero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.eth = balance
            .eth
            .checked_sub(transfer_amount.into())
            .ok_or(anyhow!("Insufficient wETH."))?;
        balance.weth_azero = balance
            .weth_azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wETH overflow."))?;
        Ok(balance)
    }

    /// Bridge wETH to ETH.
    pub fn bridge_eth_azero_to_eth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.weth_azero = balance
            .weth_azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wETH."))?;
        balance.eth = balance
            .eth
            .checked_add(transfer_amount.into())
            .ok_or(anyhow!("ETH overflow."))?;
        Ok(balance)
    }

    /// Bridge wAZERO to AZERO.
    pub fn bridge_azero_eth_to_azero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.wazero_eth = balance
            .wazero_eth
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wAZERO."))?;
        balance.azero = balance
            .azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("AZERO overflow."))?;
        Ok(balance)
    }

    /// Bridge AZERO to wAZERO.
    pub fn bridge_azero_azero_to_eth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.azero = balance
            .azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient AZERO."))?;
        balance.wazero_eth = balance
            .wazero_eth
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wAZERO overflow."))?;
        Ok(balance)
    }

    /// Bridge wETH from Ethereum to Aleph.
    pub fn bridge_weth_eth_to_azero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.weth_eth = balance
            .weth_eth
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wETH."))?;
        balance.weth_azero = balance
            .weth_azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wETH overflow."))?;
        Ok(balance)
    }

    /// Bridge wETH from Aleph to Ethereum.
    pub fn bridge_weth_azero_to_eth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.weth_azero = balance
            .weth_azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wETH."))?;
        balance.weth_eth = balance
            .weth_eth
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wETH overflow."))?;
        Ok(balance)
    }

    /// Bridge USDT from Ethereum to Aleph.
    pub fn bridge_usdt_eth_to_azero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.usdt_eth = balance
            .usdt_eth
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient USDT."))?;
        balance.usdt_azero = balance
            .usdt_azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("USDT overflow."))?;
        Ok(balance)
    }

    /// Bridge USDT from Aleph to Ethereum.
    pub fn bridge_usdt_azero_to_eth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.usdt_azero = balance
            .usdt_azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wETH."))?;
        balance.usdt_eth = balance
            .usdt_eth
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wETH overflow."))?;
        Ok(balance)
    }

    /// Bridge wAZERO from Ethereum to Aleph.
    pub fn bridge_wazero_eth_to_azero(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.wazero_eth = balance
            .wazero_eth
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wAZERO."))?;
        balance.wazero_azero = balance
            .wazero_azero
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wAZERO overflow."))?;
        Ok(balance)
    }

    /// Bridge wAZERO from Aleph to Ethereum.
    pub fn bridge_wazero_azero_to_eth(&self, transfer_amount: u128) -> Result<Self> {
        let mut balance = self.clone();
        balance.wazero_azero = balance
            .wazero_azero
            .checked_sub(transfer_amount)
            .ok_or(anyhow!("Insufficient wAZERO."))?;
        balance.wazero_eth = balance
            .wazero_eth
            .checked_add(transfer_amount)
            .ok_or(anyhow!("wAZERO overflow."))?;
        Ok(balance)
    }

    /// Check if this instance satisfies the given target.
    /// All balances of wrapped tokens must be exactly equal.
    /// Balances representing native coins must be less or equal than the target,
    /// because the target does not include unpredictable fees.
    /// Providing optional `max_eth_fee` or `max_azero_fee` will trigger additional checks -
    /// the current AZERO balance must not be less than target
    /// AZERO balance minus max fees, and similarly for ETH.
    pub fn satisfies_target(
        &self,
        target: &Self,
        max_eth_fee: Option<U256>,
        max_azero_fee: Option<u128>,
    ) -> bool {
        let mut output = self.eth <= target.eth
            && self.weth_eth == target.weth_eth
            && self.wazero_eth == target.wazero_eth
            && self.usdt_eth == target.usdt_eth
            && self.azero <= target.azero
            && self.weth_azero == target.weth_azero
            && self.wazero_azero == target.wazero_azero
            && self.usdt_azero == target.usdt_azero;
        if let Some(tolerance) = max_eth_fee {
            output = output && self.eth + tolerance >= target.eth;
        }
        if let Some(tolerance) = max_azero_fee {
            output = output && self.azero + tolerance >= target.azero;
        }
        output
    }
}
