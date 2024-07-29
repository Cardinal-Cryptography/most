use anyhow::Result;
use ethers::utils;
use log::info;

use crate::{client::Client, config::setup_test, wait::wait_for_balance_change};

/// One-way `Aleph Zero` -> `Ethereum` transfer through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// Approves the `most` contract to use the wETH funds.
/// Burns the required funds in the wETH contract on Aleph Zero.
/// Transfers `transfer_amount` of burned wETH over the bridge, unwrapping the transfer to a specified Ethereum account.
/// Waits for the transfer to complete.
/// Verifies that the correct amount of ETH is present on the Ethereum chain.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn weth_to_weth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", initial_balance);

    info!("Approve the `most` contract to use the wETH funds on the Azero chain");
    client.approve_weth_azero(transfer_amount).await?;

    info!("Request the transfer of wETH to the Ethereum chain");
    client.request_weth_transfer_azero(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_weth_azero_to_eth(transfer_amount)?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(0.into()),
        None,
        config.test_args.wait_max_minutes,
    )
    .await
}

#[tokio::test]
pub async fn usdt_to_usdt() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", initial_balance);

    info!("Approve the `most` contract to use the USDT funds on the Azero chain");
    client.approve_usdt_azero(transfer_amount).await?;

    info!("Request the transfer of USDT to the Ethereum chain");
    client.request_usdt_transfer_azero(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_usdt_azero_to_eth(transfer_amount)?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(0.into()),
        None,
        config.test_args.wait_max_minutes,
    )
    .await
}

#[tokio::test]
pub async fn wazero_to_wazero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("{:?}", initial_balance);

    info!("wrap azero");
    client.wrap_wazero(transfer_amount).await?;

    info!("Approve the `most` contract to use the wAZERO funds on the Azero chain");
    client.approve_wazero_azero(transfer_amount).await?;

    info!("Request the transfer of wAZERO to the Ethereum chain");
    client
        .request_wazero_transfer_azero(transfer_amount)
        .await?;

    info!("Wait for balance change");
    let target_balance = initial_balance
        .wrap_wazero(transfer_amount)?
        .bridge_wazero_azero_to_eth(transfer_amount)?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(0.into()),
        None,
        config.test_args.wait_max_minutes,
    )
    .await
}
