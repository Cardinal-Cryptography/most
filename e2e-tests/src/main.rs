use std::str::FromStr;

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

mod azero;
mod config;
mod eth;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const DEV_MNEMONIC: &str =
        "harsh master island dirt equip search awesome double turn crush wool grant";

    let config = config::Config::parse();

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(DEV_MNEMONIC)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();

    let eth_signed_connection = eth::signed_connection(&config.eth_node_http, wallet).await?;

    let eth_contract_addresses = eth::contract_addresses(&config.eth_contract_addresses_path)?;
    let weth_eth_address = eth_contract_addresses.weth9.parse::<Address>()?;
    println!("weth eth address: {:?}", weth_eth_address);
    let weth_abi = eth::contract_abi(&config.eth_contract_metadata_paths.weth9)?;
    let weth =
        eth::contract_from_deployed(weth_eth_address, weth_abi, eth_signed_connection.clone())?;

    let initial_balance = eth_signed_connection
        .get_balance(eth_account_address, None)
        .await?;
    println!("initial balance: {:?}", initial_balance);

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
        .ok_or(anyhow::anyhow!("Send transaction receipt not available."))?;
    println!("send receipt: {:?}", send_receipt);

    let post_transfer_eth_balance = eth_signed_connection
        .get_balance(eth_account_address, None)
        .await?;
    println!("post-transfer eth balance: {:?}", post_transfer_eth_balance);

    let most_address = eth_contract_addresses.most.parse::<Address>()?;

    let approve_args = (
        most_address,
        utils::parse_ether(config.test_args.transfer_amount)?,
    );
    println!("most address: {:?}", most_address);
    let approve_call = weth.method::<_, H256>("approve", approve_args)?;
    let approve_call = approve_call.gas(config.eth_gas_limit);
    let approve_pending_tx = approve_call.send().await?;
    let approve_receipt = approve_pending_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow::anyhow!(
            "'Approve' transaction receipt not available."
        ))?;
    println!("approve receipt: {:?}", approve_receipt);

    let most_abi = eth::contract_abi(&config.eth_contract_metadata_paths.most)?;
    let most = eth::contract_from_deployed(most_address, most_abi, eth_signed_connection.clone())?;

    let mut weth_eth_address_bytes = [0_u8; 32];
    println!(
        "weth eth address as fixed bytes: {:?}",
        weth_eth_address.as_fixed_bytes()
    );
    weth_eth_address_bytes[12..].copy_from_slice(weth_eth_address.as_fixed_bytes());
    println!("weth eth address: {:?}", weth_eth_address);
    println!("weth eth address bytes: {:?}", weth_eth_address_bytes);

    let azero_contract_addresses =
        azero::contract_addresses(&config.azero_contract_addresses_path)?;
    let weth_azero_account_id = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;
    println!("weth azero account id: {:?}", weth_azero_account_id);
    let weth_azero_address_bytes: [u8; 32] = weth_azero_account_id.into();
    println!("weth azero address bytes: {:?}", weth_azero_address_bytes);
    //let mut weth_azero_address_bytes = [0_u8; 32];
    //weth_azero_address_bytes.copy_from_slice(weth_azero_address.as_bytes());

    let add_pair_args = (weth_eth_address_bytes, weth_azero_address_bytes);
    println!("add pair args: {:?}", add_pair_args);
    let add_pair_call = most.method::<_, H256>("addPair", add_pair_args)?;
    let add_pair_pending_tx = add_pair_call.send().await?;
    let add_pair_receipt = add_pair_pending_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow::anyhow!(
            "'Add pair' transaction receipt not available."
        ))?;
    println!("add pair receipt: {:?}", add_pair_receipt);

    let azero_account_keypair = keypair_from_string("//Alice");
    let azero_account_address_bytes: [u8; 32] =
        (*azero_account_keypair.account_id()).clone().into();
    println!(
        "azero account address bytes: {:?}",
        azero_account_address_bytes
    );

    let send_request_args = (
        weth_eth_address_bytes,
        utils::parse_ether(config.test_args.transfer_amount)?,
        azero_account_address_bytes,
    );
    let send_request_call = most.method::<_, H256>("sendRequest", send_request_args)?;
    let pending_send_request_tx = send_request_call.send().await?;
    let send_request_receipt =
        pending_send_request_tx
            .confirmations(1)
            .await?
            .ok_or(anyhow::anyhow!(
                "'Send request' transaction receipt not available."
            ))?;
    println!("send request receipt: {:?}", send_request_receipt);

    Ok(())
}
