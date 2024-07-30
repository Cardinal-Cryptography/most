use aleph_client::{contract::ExecCallParams, pallets::system::SystemApi};
use anyhow::{anyhow, Result};
use ethers::{
    providers::Middleware,
    types::{Address, H256, U256, U64},
};
use log::info;

use crate::{
    azero::{self, get_psp22_balance_of},
    balance::Balance,
    config::TestContext,
    eth,
};

/// Fee required to be paid in AZERO when using the `most` contract (dictated by the GAS Oracle).
/// This is an upper bound, excess funds will be transfered back.
const BASE_FEE: u128 = 1_000_000_000_000_000_000;

/// Struct representing a single user of the `most` contract.
/// It holds two connections to Aleph and Ethereum chains, and is capable
/// of making requests and checking current balances.
pub struct Client {
    azero_signed_connection: aleph_client::SignedConnection,
    eth_signed_connection: eth::SignedConnection,
    most_eth: eth::ContractInstance,
    weth_eth: eth::ContractInstance,
    usdt_eth: eth::ContractInstance,
    wazero_eth: eth::ContractInstance,
    most_azero: azero::ContractInstance,
    weth_azero: azero::ContractInstance,
    usdt_azero: azero::ContractInstance,
    wazero_azero: azero::ContractInstance,
    azero_account_address_bytes: [u8; 32],
    eth_account_address: Address,
    eth_account_address_bytes: [u8; 32],
}

impl Client {
    pub fn new(context: TestContext) -> Self {
        let TestContext {
            azero_signed_connection,
            eth_signed_connection,
            most_eth,
            weth_eth,
            usdt_eth,
            wazero_eth,
            weth_azero,
            wazero_azero,
            usdt_azero,
            most_azero,
        } = context;
        let eth_account_address = eth_signed_connection.address();
        let mut eth_account_address_bytes = [0_u8; 32];
        eth_account_address_bytes[12..].copy_from_slice(eth_account_address.as_fixed_bytes());
        let azero_account = azero_signed_connection.signer.account_id();
        let azero_account_address_bytes: [u8; 32] = (*azero_account).clone().into();
        Self {
            azero_signed_connection,
            eth_signed_connection,
            most_eth,
            weth_eth,
            usdt_eth,
            wazero_eth,
            most_azero,
            weth_azero,
            usdt_azero,
            wazero_azero,
            azero_account_address_bytes,
            eth_account_address,
            eth_account_address_bytes,
        }
    }

    /// Check the current balances.
    pub async fn balance(&self) -> Result<Balance> {
        let azero_account = self.azero_signed_connection.signer.account_id();

        let eth = self
            .eth_signed_connection
            .get_balance(self.eth_account_address, None)
            .await?;
        let weth_eth = self
            .weth_eth
            .method::<_, u128>("balanceOf", self.eth_account_address)?
            .call()
            .await?;
        let wazero_eth = self
            .wazero_eth
            .method::<_, u128>("balanceOf", self.eth_account_address)?
            .call()
            .await?;
        let usdt_eth = self
            .usdt_eth
            .method::<_, u128>("balanceOf", self.eth_account_address)?
            .call()
            .await?;

        let azero = self
            .azero_signed_connection
            .get_free_balance(azero_account.clone(), None)
            .await;
        let weth_azero: u128 = get_psp22_balance_of(
            &self.weth_azero,
            azero_account,
            self.azero_signed_connection.clone(),
        )
        .await?;
        let usdt_azero: u128 = get_psp22_balance_of(
            &self.usdt_azero,
            azero_account,
            self.azero_signed_connection.clone(),
        )
        .await?;
        let wazero_azero: u128 = get_psp22_balance_of(
            &self.wazero_azero,
            azero_account,
            self.azero_signed_connection.clone(),
        )
        .await?;

        Ok(Balance {
            eth,
            weth_eth,
            wazero_eth,
            usdt_eth,
            azero,
            weth_azero,
            wazero_azero,
            usdt_azero,
        })
    }
}

// Wrapping native coins.
impl Client {
    /// Wrap native ETH into wETH.
    pub async fn wrap_weth(&self, transfer_amount: U256) -> Result<()> {
        info!("Attempting to wrap ETH into wETH");
        info!("Transfer amount: {}", transfer_amount);
        let wrap_receipt = eth::send_ether(
            self.eth_account_address,
            self.weth_eth.address(),
            transfer_amount,
            &self.eth_signed_connection,
        )
        .await?;

        if wrap_receipt.status.unwrap_or_default() == U64::from(1) {
            info!("Successfully wrapped {} ETH", transfer_amount);
            Ok(())
        } else {
            Err(anyhow!("Failed to wrap ETH: {:?}", wrap_receipt))
        }
    }

    /// Wrap native AZERO into wAZERO.
    pub async fn wrap_wazero(&self, transfer_amount: u128) -> Result<()> {
        info!("Attempting to wrap Azero into wAzero");
        info!("Transfer amount: {}", transfer_amount);
        let deposit_info = self
            .wazero_azero
            .exec0(
                &self.azero_signed_connection,
                "WrappedAZERO::deposit",
                ExecCallParams::new().value(transfer_amount),
            )
            .await?;
        info!("`deposit` tx info: {:?}", deposit_info);
        Ok(())
    }
}

// Approving the `most` contract to use wrapped funds (Aleph).
impl Client {
    /// Approve the `most` contract to use wrapped funds in the given contract.
    async fn approve_azero(
        &self,
        contract: &azero::ContractInstance,
        transfer_amount: u128,
    ) -> Result<()> {
        let approve_args = [
            self.most_azero.address().to_string(),
            transfer_amount.to_string(),
        ];
        let approve_info = contract
            .exec(
                &self.azero_signed_connection,
                "PSP22::approve",
                &approve_args,
                Default::default(),
            )
            .await?;
        info!("`approve` tx info: {:?}", approve_info);
        Ok(())
    }

    // Approve the `most` contract to use the wETH funds.
    pub async fn approve_weth_azero(&self, transfer_amount: u128) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the wETH funds",
            transfer_amount
        );
        self.approve_azero(&self.weth_azero, transfer_amount).await
    }

    // Approve the `most` contract to use the USDT funds.
    pub async fn approve_usdt_azero(&self, transfer_amount: u128) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the USDT funds",
            transfer_amount
        );
        self.approve_azero(&self.usdt_azero, transfer_amount).await
    }

    // Approve the `most` contract to use the wAZERO funds.
    pub async fn approve_wazero_azero(&self, transfer_amount: u128) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the wAZERO funds",
            transfer_amount
        );
        self.approve_azero(&self.wazero_azero, transfer_amount)
            .await
    }
}

// Approving the `most` contract to use wrapped funds (Aleph).
impl Client {
    /// Approve the `most` contract to use wrapped funds in the given contract.
    async fn approve_eth(
        &self,
        contract: &eth::ContractInstance,
        transfer_amount: U256,
    ) -> Result<()> {
        let approve_args = (self.most_eth.address(), transfer_amount);
        let approve_receipt = eth::contract_exec(contract, "approve", approve_args).await?;

        if approve_receipt.status.unwrap_or_default() == U64::from(1) {
            info!("Successfully approved the `most` contract to use wrapped token");
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to approve the `most` contract to use wrapped token: {:?}",
                approve_receipt
            ))
        }
    }

    // Approve the `most` contract to use the wETH funds.
    pub async fn approve_weth_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the wETH funds",
            transfer_amount
        );
        self.approve_eth(&self.weth_eth, transfer_amount).await
    }

    // Approve the `most` contract to use the wAZERO funds.
    pub async fn approve_wazero_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the wAZERO funds",
            transfer_amount
        );
        self.approve_eth(&self.wazero_eth, transfer_amount).await
    }

    // Approve the `most` contract to use the USDT funds.
    pub async fn approve_usdt_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to approve the 'most' contract to use {} of the USDT funds",
            transfer_amount
        );
        self.approve_eth(&self.usdt_eth, transfer_amount).await
    }
}

// Requesting cross-chain transfers (Aleph).
impl Client {
    // Request cross-chain transfer of AZERO to wAZERO from Aleph to Ethereum.
    pub async fn request_azero_transfer_azero(&self, transfer_amount: u128) -> Result<()> {
        let send_request_args = [
            transfer_amount.to_string(),
            azero::bytes32_to_string(&self.eth_account_address_bytes),
        ];
        let send_request_info = self
            .most_azero
            .exec(
                &self.azero_signed_connection,
                "send_request_native_azero",
                &send_request_args,
                ExecCallParams::new().value(BASE_FEE),
            )
            .await?;
        info!(
            "`send_request_native_azero` tx info: {:?}",
            send_request_info
        );
        Ok(())
    }

    // Request cross-chain transfer of wETH to ETH from Aleph to Ethereum.
    pub async fn request_eth_transfer_azero(&self, transfer_amount: u128) -> Result<()> {
        let send_request_args = [
            transfer_amount.to_string(),
            azero::bytes32_to_string(&self.eth_account_address_bytes),
        ];
        let send_request_info = self
            .most_azero
            .exec(
                &self.azero_signed_connection,
                "send_request_native_ether",
                &send_request_args,
                ExecCallParams::new().value(BASE_FEE),
            )
            .await?;
        info!(
            "`send_request_native_ether` tx info: {:?}",
            send_request_info
        );
        Ok(())
    }

    // Request cross-chain transfer of the given wrapped token from Aleph to Ethereum.
    async fn request_transfer_azero(
        &self,
        contract: &azero::ContractInstance,
        transfer_amount: u128,
    ) -> Result<()> {
        let contract_address_bytes: [u8; 32] = (contract.address()).clone().into();
        let send_request_args = [
            azero::bytes32_to_string(&contract_address_bytes),
            transfer_amount.to_string(),
            azero::bytes32_to_string(&self.eth_account_address_bytes),
        ];
        let send_request_info = self
            .most_azero
            .exec(
                &self.azero_signed_connection,
                "send_request",
                &send_request_args,
                ExecCallParams::new().value(BASE_FEE),
            )
            .await?;
        info!("`send_request` tx info: {:?}", send_request_info);
        Ok(())
    }

    // Request cross-chain transfer of wETH from Aleph to Ethereum.
    pub async fn request_weth_transfer_azero(&self, transfer_amount: u128) -> Result<()> {
        self.request_transfer_azero(&self.weth_azero, transfer_amount)
            .await
    }

    // Request cross-chain transfer of USDT from Aleph to Ethereum.
    pub async fn request_usdt_transfer_azero(&self, transfer_amount: u128) -> Result<()> {
        self.request_transfer_azero(&self.usdt_azero, transfer_amount)
            .await
    }

    // Request cross-chain transfer of wAZERO from Aleph to Ethereum.
    pub async fn request_wazero_transfer_azero(&self, transfer_amount: u128) -> Result<()> {
        self.request_transfer_azero(&self.wazero_azero, transfer_amount)
            .await
    }
}

// Requesting cross-chain transfers (Ethereum)
impl Client {
    // Request cross-chain transfer of wAZERO to AZERO from Ethereum to Aleph.
    pub async fn request_azero_transfer_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to transfer {} of the wAZERO funds",
            transfer_amount
        );
        let send_request_args = (transfer_amount, self.azero_account_address_bytes);
        let call = self
            .most_eth
            .method::<_, H256>("sendRequestAzeroToNative", send_request_args)?;
        let pending_tx = call.send().await?;
        let send_request_receipt = pending_tx
            .confirmations(1)
            .await?
            .ok_or(anyhow!("tx receipt not available."))?;

        if send_request_receipt.status.unwrap_or_default() == U64::from(1) {
            info!("Successfully requested the transfer to the Aleph Zero chain",);
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to request the transfer to the Aleph Zero chain: {:?}",
                send_request_receipt
            ))
        }
    }

    // Request cross-chain transfer of ETH to wETH from Ethereum to Aleph.
    pub async fn request_eth_transfer_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to transfer {} of the ETH funds",
            transfer_amount
        );
        let send_request_args = (self.azero_account_address_bytes,);
        let call = self
            .most_eth
            .method::<_, H256>("sendRequestNative", send_request_args)?
            .value(transfer_amount);
        let pending_tx = call.send().await?;
        let send_request_receipt = pending_tx
            .confirmations(1)
            .await?
            .ok_or(anyhow!("tx receipt not available."))?;

        if send_request_receipt.status.unwrap_or_default() == U64::from(1) {
            info!("Successfully requested the transfer to the Aleph Zero chain",);
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to request the transfer to the Aleph Zero chain: {:?}",
                send_request_receipt
            ))
        }
    }

    // Request cross-chain transfer of the given wrapped token from Ethereum to Aleph.
    async fn request_transfer_eth(
        &self,
        contract: &eth::ContractInstance,
        transfer_amount: U256,
    ) -> Result<()> {
        let mut contract_address_bytes = [0_u8; 32];
        contract_address_bytes[12..].copy_from_slice(contract.address().as_fixed_bytes());
        let send_request_args = (
            contract_address_bytes,
            transfer_amount,
            self.azero_account_address_bytes,
        );
        let send_request_receipt =
            eth::contract_exec(&self.most_eth, "sendRequest", send_request_args).await?;
        if send_request_receipt.status.unwrap_or_default() == U64::from(1) {
            info!("Successfully requested the transfer to the Aleph Zero chain",);
            Ok(())
        } else {
            Err(anyhow!(
                "Failed to request the transfer to the Aleph Zero chain: {:?}",
                send_request_receipt
            ))
        }
    }

    // Request cross-chain transfer of wETH from Ethereum to Aleph.
    pub async fn request_weth_transfer_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to transfer {} of the wETH funds",
            transfer_amount
        );
        self.request_transfer_eth(&self.weth_eth, transfer_amount)
            .await
    }

    // Request cross-chain transfer of USDT from Ethereum to Aleph.
    pub async fn request_usdt_transfer_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to transfer {} of the USDT funds",
            transfer_amount
        );
        self.request_transfer_eth(&self.usdt_eth, transfer_amount)
            .await
    }

    // Request cross-chain transfer of wAZERO from Ethereum to Aleph.
    pub async fn request_wazero_transfer_eth(&self, transfer_amount: U256) -> Result<()> {
        info!(
            "Attempting to transfer {} of the wAZERO funds",
            transfer_amount
        );
        self.request_transfer_eth(&self.wazero_eth, transfer_amount)
            .await
    }
}
