use std::str::FromStr;

use aleph_client::{contract::ContractInstance, keypair_from_string, sp_runtime::AccountId32};
use ethers::{
    middleware::Middleware,
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    utils,
};
use log::info;

use crate::{azero, config::setup_test, eth, wait::wait_for_balance_change};

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
pub async fn azero_to_eth() -> anyhow::Result<()> {
    let config = setup_test();

    let azero_contract_addresses =
        azero::contract_addresses(&config.azero_contract_addresses_path)?;
    let most_address = AccountId32::from_str(&azero_contract_addresses.most)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;
    let weth_azero_address = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;

    let weth_azero = ContractInstance::new(
        weth_azero_address.clone(),
        &config.contract_metadata_paths.azero_token,
    )?;

    let azero_account_keypair = keypair_from_string(&config.azero_account_seed);
    let azero_signed_connection =
        azero::signed_connection(&config.azero_node_ws, &azero_account_keypair).await;

    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)?.as_u128();

    let approve_args = [most_address.to_string(), transfer_amount.to_string()];

    let approve_info = weth_azero
        .contract_exec(&azero_signed_connection, "PSP22::approve", &approve_args)
        .await?;
    info!("`approve` tx info: {:?}", approve_info);

    let most = ContractInstance::new(most_address, &config.contract_metadata_paths.azero_most)?;

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(&*config.eth_mnemonic)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();
    let mut eth_account_address_bytes = [0_u8; 32];
    eth_account_address_bytes[12..].copy_from_slice(eth_account_address.as_fixed_bytes());

    let eth_connection = eth::connection(&config.eth_node_http).await?;

    let balance_pre_transfer = eth_connection
        .get_balance(eth_account_address, None)
        .await?;
    info!("ETH balance pre transfer: {:?}", balance_pre_transfer);

    let weth_azero_address_bytes: [u8; 32] = weth_azero_address.into();
    let send_request_args = [
        azero::bytes32_to_string(&weth_azero_address_bytes),
        transfer_amount.to_string(),
        azero::bytes32_to_string(&eth_account_address_bytes),
    ];

    let send_request_info = most
        .contract_exec_value(
            &azero_signed_connection,
            "send_request",
            &send_request_args,
            100_000_000_000_000,
        )
        .await?;
    info!("`send_request` tx info: {:?}", send_request_info);

    let get_current_balance = || async {
        let balance_current = eth_connection
            .get_balance(eth_account_address, None)
            .await
            .map_err(|e| anyhow::anyhow!("Cannot read ETH balance: {:?}", e))?
            .as_u128();
        Ok::<_, anyhow::Error>(balance_current)
    };

    wait_for_balance_change(
        transfer_amount,
        balance_pre_transfer.as_u128(),
        get_current_balance,
        config.test_args.wait_max_minutes,
    )
    .await
}
