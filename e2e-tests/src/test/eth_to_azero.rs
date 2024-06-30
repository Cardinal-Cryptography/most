use std::str::FromStr;

use aleph_client::{contract::ContractInstance, keypair_from_string, sp_runtime::AccountId32};
use anyhow::{anyhow, Result};
use ethers::{core::types::Address, types::U64, utils};
use log::info;

use crate::{
    azero,
    azero::get_psp22_balance_of,
    config::{setup_test, TestContext},
    eth, test,
    wait::wait_for_balance_change,
};

/// One-way `Ethereum` -> `Aleph Zero` transfer through `most`.
/// Wraps the required funds into wETH for an Ethereum account.
/// Approves the `most` contract to use the wETH funds.
/// Transfers `transfer_amount` of wETH to a specified Aleph Zero account over the bridge.
/// Waits for the transfer to complete - bottlenecked by Ethereum finalization.
/// Verifies that the correct amount of wETH is present on the Aleph Zero chain.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn eth_to_azero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;

    let TestContext {
        azero_signed_connection,
        eth_signed_connection,
        weth_eth,
        most_eth,
        weth_azero,
        ..
    } = test_context;

    info!("Running test of Ethereum -> Aleph Zero ETH transfer...");

    let eth_account_address = eth_signed_connection.address();
    let azero_account = azero_signed_connection.signer.account_id();

    let mut weth_eth_address_bytes = [0_u8; 32];
    weth_eth_address_bytes[12..].copy_from_slice(weth_eth.address().as_fixed_bytes());
    let azero_account_address_bytes: [u8; 32] = (*azero_account).clone().into();

    // Wrap some ETH into wETH
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)?;
    let wrap_receipt = eth::send_ether(
        eth_account_address,
        weth_eth.address(),
        transfer_amount + 100,
        &eth_signed_connection,
    )
    .await?;

    if wrap_receipt.status.unwrap_or_default() == U64::from(1) {
        info!(
            "Successfully wrapped {} ETH into wETH",
            transfer_amount + 100
        );
    } else {
        return Err(anyhow!("Failed to wrap ETH into wETH: {:?}", wrap_receipt));
    }

    // Approve the `most` contract to use the wETH funds
    let approve_args = (most_eth.address(), transfer_amount);
    let approve_receipt = eth::call_contract_method(weth_eth, "approve", approve_args).await?;

    if approve_receipt.status.unwrap_or_default() == U64::from(1) {
        info!("Successfully approved the `most` contract to use wETH");
    } else {
        return Err(anyhow!(
            "Failed to approve the `most` contract to use wETH: {:?}",
            approve_receipt
        ));
    }

    let balance_pre_transfer: u128 =
        get_psp22_balance_of(&weth_azero, azero_account, azero_signed_connection.clone()).await?;

    info!(
        "wETH (Aleph Zero) balance pre transfer: {:?}",
        balance_pre_transfer
    );

    // Request the transfer of wETH to the Aleph Zero chain
    let send_request_args = (
        weth_eth_address_bytes,
        transfer_amount,
        azero_account_address_bytes,
    );
    let send_request_receipt =
        eth::call_contract_method(most_eth, "sendRequest", send_request_args).await?;
    if send_request_receipt.status.unwrap_or_default() == U64::from(1) {
        info!(
            "Successfully requested the transfer of {} wETH to the Aleph Zero chain",
            transfer_amount
        );
    } else {
        return Err(anyhow!(
            "Failed to request the transfer of wETH to the Aleph Zero chain: {:?}",
            send_request_receipt
        ));
    }

    let get_current_balance = || async {
        Ok(
            get_psp22_balance_of(&weth_azero, azero_account, azero_signed_connection.clone())
                .await?,
        )
    };

    wait_for_balance_change(
        transfer_amount.as_u128(),
        balance_pre_transfer,
        get_current_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}
