use aleph_client::contract::ExecCallParams;
use anyhow::{Error, Result};
use ethers::utils;
use log::info;

use crate::{
    azero,
    config::{setup_test, TestContext},
    wait::wait_for_balance_change,
};

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
pub async fn azero_to_eth() -> Result<()> {
    let config = setup_test();
    let test_context = config.create_test_context().await?;

    let TestContext {
        azero_signed_connection,
        eth_signed_connection,
        weth_azero,
        weth_eth,
        most_azero,
        ..
    } = test_context;

    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)?.as_u128();

    let approve_args = [
        most_azero.address().to_string(),
        transfer_amount.to_string(),
    ];

    let approve_info = weth_azero
        .exec(
            &azero_signed_connection,
            "PSP22::approve",
            &approve_args,
            Default::default(),
        )
        .await?;
    info!("`approve` tx info: {:?}", approve_info);

    let eth_account_address = eth_signed_connection.address();
    let mut eth_account_address_bytes = [0_u8; 32];
    eth_account_address_bytes[12..].copy_from_slice(eth_account_address.as_fixed_bytes());

    let balance_pre_transfer = weth_eth
        .method::<_, u128>("balanceOf", eth_account_address)?
        .call()
        .await?;
    info!("ETH balance pre transfer: {:?}", balance_pre_transfer);

    let weth_azero_address_bytes: [u8; 32] = (*weth_azero.address()).clone().into();
    let send_request_args = [
        azero::bytes32_to_string(&weth_azero_address_bytes),
        transfer_amount.to_string(),
        azero::bytes32_to_string(&eth_account_address_bytes),
    ];

    let send_request_info = most_azero
        .exec(
            &azero_signed_connection,
            "send_request",
            &send_request_args,
            ExecCallParams::new().value(10_000_000_000_000_000),
        )
        .await?;
    info!("`send_request` tx info: {:?}", send_request_info);

    let get_current_balance = || async {
        let balance_current = weth_eth
            .method::<_, u128>("balanceOf", eth_account_address)?
            .call()
            .await?;
        Ok::<u128, Error>(balance_current)
    };

    wait_for_balance_change(
        transfer_amount,
        balance_pre_transfer,
        get_current_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}
