use std::str::FromStr;

use ethers::signers::{coins_bip39::English, MnemonicBuilder, Signer};
use log::info;

use aleph_client::{contract::ContractInstance, sp_runtime::AccountId32, keypair_from_string};

use crate::{azero, config::setup_test, eth};

#[tokio::test]
pub async fn azero_to_eth() -> anyhow::Result<()> {
    let config = setup_test();

    let azero_contract_addresses =
        azero::contract_addresses(&config.azero_contract_addresses_path)?;
    let most_address = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;
    let weth_azero_address = AccountId32::from_str(&azero_contract_addresses.weth)
        .map_err(|e| anyhow::anyhow!("Cannot parse account id from string: {:?}", e))?;

    let most = ContractInstance::new(most_address, &config.contract_metadata_paths.azero_most)?;

    let azero_account_keypair = keypair_from_string(&config.azero_account_seed);
    let azero_signed_connection = azero::signed_connection(&config.azero_node_ws, &azero_account_keypair).await;

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(&*config.eth_mnemonic)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();
    let mut eth_account_address_bytes = [0_u8; 32];
    eth_account_address_bytes[12..].copy_from_slice(eth_account_address.as_fixed_bytes());

    let eth_connection = eth::connection(&config.eth_node_http).await?;

    let balance_pre_unwrap = eth_connection
        .get_balance(eth_account_address, None)
        .await?;

    let send_request_args = [weth_azero_address.to_string(), config.test_args.transfer_amount.to_string(), azero::bytes32_to_string(&eth_account_address_bytes)];

    let send_request_info = most.contract_exec(&azero_signed_connection, "send_request", &send_request_args).await?;
    info!("`send_request` tx info: {:?}", send_request_info);

    let balance_post_unwrap = eth_connection
        .get_balance(eth_account_address, None)
        .await?;

    assert_eq!(balance_post_unwrap - balance_pre_unwrap, config.test_args.transfer_amount);

    Ok(())
}