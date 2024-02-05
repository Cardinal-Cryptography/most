use std::{env, str::FromStr};

use aleph_client::{keypair_from_string, sp_runtime::AccountId32};
use anyhow;
use clap::Parser;
use ethers::{
    core::types::{Address, TransactionRequest, U256},
    middleware::Middleware,
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::H256,
    utils,
};
use log::info;

mod azero;
mod config;
mod eth;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const DEV_MNEMONIC: &str =
        "harsh master island dirt equip search awesome double turn crush wool grant";

    let config = config::Config::parse();

    env::set_var("RUST_LOG", config.rust_log.as_str());
    env_logger::init();

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(DEV_MNEMONIC)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();

    let eth_signed_connection = eth::signed_connection(&config.eth_node_http, wallet).await?;

    let eth_contract_addresses = eth::contract_addresses(&config.eth_contract_addresses_path)?;
    let weth_eth_address = eth_contract_addresses.weth9.parse::<Address>()?;

    let weth_abi = eth::contract_abi(&config.eth_contract_metadata_paths.weth9)?;
    let weth =
        eth::contract_from_deployed(weth_eth_address, weth_abi, eth_signed_connection.clone())?;

    let send_tx = TransactionRequest::new()
        .to(weth_eth_address)
        .value(U256::from(utils::parse_ether(
            config.test_args.transfer_amount + 100,
        )?))
        .from(eth_account_address);
    let send_receipt = eth_signed_connection
        .send_transaction(send_tx, None)
        .await?
        .await?
        .ok_or(anyhow::anyhow!("Send tx receipt not available."))?;
    info!("Send tx receipt: {:?}", send_receipt);

    let most_address = eth_contract_addresses.most.parse::<Address>()?;

    let approve_args = (
        most_address,
        utils::parse_ether(config.test_args.transfer_amount)?,
    );

    let approve_call = weth.method::<_, H256>("approve", approve_args)?;
    let approve_call = approve_call.gas(config.eth_gas_limit);
    let approve_pending_tx = approve_call.send().await?;
    let approve_receipt = approve_pending_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow::anyhow!("'approve' tx receipt not available."))?;
    info!("'Approve' tx receipt: {:?}", approve_receipt);

    let most_abi = eth::contract_abi(&config.eth_contract_metadata_paths.most)?;
    let most = eth::contract_from_deployed(most_address, most_abi, eth_signed_connection.clone())?;

    let mut weth_eth_address_bytes = [0_u8; 32];
    weth_eth_address_bytes[12..].copy_from_slice(weth_eth_address.as_fixed_bytes());

    let azero_contract_addresses =
        azero::contract_addresses(&config.azero_contract_addresses_path)?;
    let _weth_azero_account_id = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;

    let azero_account_keypair = keypair_from_string("//Alice");
    let azero_account_address_bytes: [u8; 32] =
        (*azero_account_keypair.account_id()).clone().into();

    let send_request_args = (
        weth_eth_address_bytes,
        utils::parse_ether(config.test_args.transfer_amount)?,
        azero_account_address_bytes,
    );
    let send_request_call = most.method::<_, H256>("sendRequest", send_request_args)?;
    let pending_send_request_tx = send_request_call.send().await?;
    let send_request_receipt = pending_send_request_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow::anyhow!("'sendRequest' tx receipt not available."))?;
    info!("'sendRequest' tx receipt: {:?}", send_request_receipt);

    Ok(())
}
