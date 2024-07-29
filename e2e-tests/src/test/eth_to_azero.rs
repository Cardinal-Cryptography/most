use anyhow::Result;
use ethers::utils;
use log::info;

use crate::{client::Client, config::setup_test, wait::wait_for_balance_change};

/// One-way `Ethereum` -> `Aleph Zero` transfer through `most`.
/// Wraps the required funds into wETH for an Ethereum account.
/// Approves the `most` contract to use the wETH funds.
/// Transfers `transfer_amount` of wETH to a specified Aleph Zero account over the bridge.
/// Waits for the transfer to complete - bottlenecked by Ethereum finalization.
/// Verifies that the correct amount of wETH is present on the Aleph Zero chain.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn weth_to_weth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", client.balance().await?);

    info!("Wrap some ETH into wETH");
    client.wrap_weth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Approve the `most` contract to use the wETH funds");
    client.approve_weth_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Request the transfer of wETH to the Aleph Zero chain");
    client.request_weth_transfer_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Wait for balance change");

    let target_balance = initial_balance
        .wrap_weth(transfer_amount.as_u128())?
        .bridge_weth_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}

#[tokio::test]
pub async fn usdt_to_usdt() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)? / 20_u128;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", client.balance().await?);

    info!("Approve the `most` contract to use the USDT funds");
    client.approve_usdt_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Request the transfer of USDT to the Aleph Zero chain");
    client.request_usdt_transfer_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Wait for balance change");

    let target_balance = initial_balance.bridge_usdt_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}

#[tokio::test]
pub async fn wazero_to_wazero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount =
        utils::parse_ether(config.test_args.transfer_amount)?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", initial_balance);

    info!("Approve the `most` contract to use the wAZERO funds on the Ethereum chain");
    client.approve_wazero_eth(transfer_amount).await?;

    info!("Request the transfer of wAZERO to the Azero chain");
    client.request_wazero_transfer_eth(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_wazero_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}
