use std::{ops::AddAssign, str::FromStr};

use aleph_client::{contract::ContractInstance, keypair_from_string, sp_runtime::AccountId32};
use ethers::{
    core::types::Address,
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    utils,
};
use log::info;

use crate::{azero, config::setup_test, eth};

/// One-way `Ethereum` -> `Aleph Zero` transfer through `most`.
/// Wraps the required funds into wETH for an Ethereum account.
/// Approves the `most` contract to use the wETH funds.
/// Transfers `transfer_amount` of wETH to a specified Aleph Zero account over the bridge.
/// Waits for the transfer to complete - bottlenecked by Ethereum finalization.
/// Verifies that the correct amount of wETH is present on the Aleph Zero chain.
/// It relies on all the relevant contracts being deployed on both ends and the (wETH_ETH:wETH_AZERO) pair having been added to `most`.
#[tokio::test]
pub async fn eth_to_azero() -> anyhow::Result<()> {
    let config = setup_test();

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(&*config.eth_mnemonic)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();

    let eth_signed_connection = eth::signed_connection(&config.eth_node_http, wallet).await?;

    let eth_contract_addresses = eth::contract_addresses(&config.eth_contract_addresses_path)?;
    let weth_eth_address = eth_contract_addresses.weth9.parse::<Address>()?;

    let weth_abi = eth::contract_abi(&config.contract_metadata_paths.eth_weth9)?;
    let weth = eth::contract_from_deployed(weth_eth_address, weth_abi, &eth_signed_connection)?;

    let transfer_amount = utils::parse_ether(config.test_args.transfer_amount)?;
    let send_receipt = eth::send_tx(
        eth_account_address,
        weth_eth_address,
        transfer_amount + 100,
        &eth_signed_connection,
    )
    .await?;
    info!("Send tx receipt: {:?}", send_receipt);

    let most_address = eth_contract_addresses.most.parse::<Address>()?;

    let approve_args = (
        most_address,
        utils::parse_ether(config.test_args.transfer_amount)?,
    );

    let approve_receipt =
        eth::call_contract_method(weth, "approve", config.eth_gas_limit, approve_args).await?;
    info!("'Approve' tx receipt: {:?}", approve_receipt);

    let azero_contract_addresses =
        azero::contract_addresses(&config.azero_contract_addresses_path)?;
    let weth_azero_account_id = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;

    let weth_azero_contract = ContractInstance::new(
        weth_azero_account_id,
        &config.contract_metadata_paths.azero_token,
    )?;

    let azero_connection = azero::connection(&config.azero_node_ws).await;

    let azero_account_keypair = keypair_from_string("//Alice");
    let azero_account = azero_account_keypair.account_id();

    let balance_pre_transfer: u128 = weth_azero_contract
        .contract_read(
            &azero_connection,
            "PSP22::balance_of",
            &[azero_account.to_string()],
        )
        .await?;
    info!(
        "wETH (Aleph Zero) balance pre transfer: {:?}",
        balance_pre_transfer
    );

    let most_abi = eth::contract_abi(&config.contract_metadata_paths.eth_most)?;
    let most = eth::contract_from_deployed(most_address, most_abi, &eth_signed_connection)?;

    let mut weth_eth_address_bytes = [0_u8; 32];
    weth_eth_address_bytes[12..].copy_from_slice(weth_eth_address.as_fixed_bytes());

    let azero_account_address_bytes: [u8; 32] = (*azero_account).clone().into();

    let send_request_args = (
        weth_eth_address_bytes,
        utils::parse_ether(config.test_args.transfer_amount)?,
        azero_account_address_bytes,
    );
    let send_request_receipt =
        eth::call_contract_method(most, "sendRequest", config.eth_gas_limit, send_request_args)
            .await?;
    info!("'sendRequest' tx receipt: {:?}", send_request_receipt);

    let tick = tokio::time::Duration::from_secs(30_u64);
    let wait_max = tokio::time::Duration::from_secs(60_u64 * config.test_args.wait_max_minutes);

    info!(
        "Waiting a max. of {:?} minutes for finalization",
        config.test_args.wait_max_minutes
    );

    let mut wait = tokio::time::Duration::from_secs(0_u64);

    while wait <= wait_max {
        tokio::time::sleep(tick).await;
        wait.add_assign(tick);
        let balance_current: u128 = weth_azero_contract
            .contract_read(
                &azero_connection,
                "PSP22::balance_of",
                &[azero_account.to_string()],
            )
            .await?;
        let balance_change = balance_current - balance_pre_transfer;
        if balance_change == transfer_amount.as_u128() {
            info!(
                "wETH (Aleph Zero) required balance change detected: {:?}",
                balance_change
            );
            return Ok(());
        }
        if wait.as_secs() % 60 == 0 {
            info!("minutes elapsed: {:?}", wait.as_secs() / 60)
        }
    }

    Err(anyhow::anyhow!(
        "Failed to detect required wETH (Aleph Zero) balance change of: {:?}",
        transfer_amount
    ))
}