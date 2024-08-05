use anyhow::Result;
use ethers::utils;
use log::info;

use crate::{client::Client, config::setup_test, wait::wait_for_balance_change};

/// One-way `Ethereum` -> `Aleph Zero` transfer of wETH through `most`.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn weth_to_weth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Wrap native ETH into wETH");
    client.wrap_weth(transfer_amount).await?;

    info!("Approve the `most` contract to use the wETH funds on the Ethereum chain");
    client.approve_weth_eth(transfer_amount).await?;

    info!("Request the transfer of wETH to the Aleph chain");
    client.request_weth_transfer_eth(transfer_amount).await?;

    info!("Wait for balance change");

    let target_balance = initial_balance
        .wrap_weth(transfer_amount.as_u128())?
        .bridge_weth_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(transfer_amount / 100),
        Some(0),
        config.test_args.wait_max_minutes,
    )
    .await
}

/// One-way `Ethereum` -> `Aleph Zero` transfer of USDT through `most`.
/// It relies on all the relevant contracts being deployed on both ends and the (USDT_ETH:USDT_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn usdt_to_usdt() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the USDT funds on the Ethereum chain");
    client.approve_usdt_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Request the transfer of USDT to the Aleph chain");
    client.request_usdt_transfer_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Wait for balance change");

    let target_balance = initial_balance.bridge_usdt_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(transfer_amount / 100),
        Some(0),
        config.test_args.wait_max_minutes,
    )
    .await
}

/// One-way `Ethereum` -> `Aleph Zero` transfer of wAZERO through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// It relies on all the relevant contracts being deployed on both ends and the (wAZERO_ETH:wAZERO_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn wazero_to_wazero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the wAZERO funds on the Ethereum chain");
    client.approve_wazero_eth(transfer_amount).await?;

    info!("Request the transfer of wAZERO to the Aleph chain");
    client.request_wazero_transfer_eth(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_wazero_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(transfer_amount / 100),
        Some(0),
        config.test_args.wait_max_minutes,
    )
    .await
}

/// One-way `Ethereum` -> `Aleph Zero` transfer of ETH to wETH through `most`.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn eth_to_weth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Request the transfer of ETH to the Aleph Zero chain");
    client.request_eth_transfer_eth(transfer_amount).await?;
    info!("{:?}", client.balance().await?);

    info!("Wait for balance change");

    let target_balance = initial_balance.bridge_eth_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(transfer_amount / 100),
        Some(0),
        config.test_args.wait_max_minutes,
    )
    .await
}

/// One-way `Ethereum` -> `Aleph Zero` transfer of wAZERO to AZERO through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// It relies on all the relevant contracts being deployed on both ends and the (wAZERO_ETH:wAZERO_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn wazero_to_azero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?;
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the wAZERO funds on the Ethereum chain");
    client.approve_wazero_eth(transfer_amount).await?;

    info!("Request the transfer of wAZERO to the Aleph chain");
    client.request_azero_transfer_eth(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_azero_eth_to_azero(transfer_amount.as_u128())?;
    info!("Target balance: {:?}", target_balance);

    let get_current_balance = || async { client.balance().await };
    wait_for_balance_change(
        get_current_balance,
        target_balance,
        Some(transfer_amount / 100),
        Some(0),
        config.test_args.wait_max_minutes,
    )
    .await
}
