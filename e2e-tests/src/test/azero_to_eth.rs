use anyhow::Result;
use ethers::utils;
use log::info;

use crate::{client::Client, config::setup_test, wait::wait_for_balance_change};

/// One-way `Aleph Zero` -> `Ethereum` transfer of wETH through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// 1. Approves the `most` contract to use the wETH funds.
/// 2. Transfers `transfer_amount` of burned wETH over the bridge.
/// 3. Waits for the transfer to complete.
/// Finally, it checks if every account has proper balance.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn weth_to_weth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the wETH funds on the Aleph chain");
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

/// One-way `Aleph Zero` -> `Ethereum` transfer of USDT through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// 1. Approves the `most` contract to use the USDT funds.
/// 2. Transfers `transfer_amount` of burned USDT over the bridge.
/// 3. Waits for the transfer to complete.
/// Finally, it checks if every account has proper balance.
/// It relies on all the relevant contracts being deployed on both ends and the (USDT_ETH:USDT_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn usdt_to_usdt() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the USDT funds on the Aleph chain");
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

/// One-way `Aleph Zero` -> `Ethereum` transfer of wAZERO through `most`.
/// 1. Wraps AZERO into wAZERO, and approves the `most` contract to use the wAZERO funds.
/// 2. Transfers `transfer_amount` of burned wAZERO over the bridge.
/// 3. Waits for the transfer to complete.
/// Finally, it checks if every account has proper balance.
/// It relies on all the relevant contracts being deployed on both ends and the (wAZERO_ETH:wAZERO_AZERO) pair having been added to `most`.
#[tokio::test]
#[ignore]
pub async fn wazero_to_wazero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Wrap native AZERO into wAZERO");
    client.wrap_wazero(transfer_amount).await?;

    info!("Approve the `most` contract to use the wAZERO funds on the Aleph chain");
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

/// One-way `Aleph Zero` -> `Ethereum` transfer of wETH to ETH through `most`.
/// Requires a prior transaction in the other direction to have completed.
/// This is easily done by running the test for the other direction first.
/// 1. Approves the `most` contract to use the wETH funds.
/// 2. Transfers `transfer_amount` of burned wETH over the bridge, the Ethereum account receives native ETH.
/// 3. Waits for the transfer to complete.
/// Finally, it checks if every account has proper balance.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn weth_to_eth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Approve the `most` contract to use the wETH funds on the Aleph chain");
    client.approve_weth_azero(transfer_amount).await?;

    info!("Request the transfer of wETH to the Ethereum chain");
    client.request_eth_transfer_azero(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_eth_azero_to_eth(transfer_amount)?;
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

/// One-way `Aleph Zero` -> `Ethereum` transfer of AZERO to wAZERO through `most`.
/// 1. Transfers `transfer_amount` of native AZERO over the bridge.
/// 2. Waits for the transfer to complete.
/// Finally, it checks if every account has proper balance.
/// It relies on all the relevant contracts being deployed on both ends and the (wAZERO_ETH:wAZERO_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn azero_to_wazero() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;
    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount.clone())?.as_u128();
    let client = Client::new(test_context);
    let initial_balance = client.balance().await?;

    info!("Initial balance: {:?}", initial_balance);

    info!("Request the transfer of AZERO to the Ethereum chain");
    client.request_azero_transfer_azero(transfer_amount).await?;

    info!("Wait for balance change");
    let target_balance = initial_balance.bridge_azero_azero_to_eth(transfer_amount)?;
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
