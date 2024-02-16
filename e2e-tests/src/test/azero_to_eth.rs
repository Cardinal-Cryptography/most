use std::str::FromStr;

use aleph_client::{
    contract::ContractInstance, keypair_from_string, sp_runtime::AccountId32, utility::BlocksApi,
};
use ethers::{
    middleware::Middleware,
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    utils,
};
use log::info;

use crate::{azero, config::setup_test, eth};

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

    loop {
        if azero_signed_connection.get_finalized_block_hash().await? == approve_info.block_hash {
            break;
        }
    }

    let most = ContractInstance::new(most_address, &config.contract_metadata_paths.azero_most)?;

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(&*config.eth_mnemonic)
        .index(config.eth_dev_account_index)?
        .build()?;
    let eth_account_address = wallet.address();
    let mut eth_account_address_bytes = [0_u8; 32];
    eth_account_address_bytes[12..].copy_from_slice(eth_account_address.as_fixed_bytes());

    //let eth_connection = eth::connection(&config.eth_node_http).await?;

    //let balance_pre_unwrap = eth_connection
    //    .get_balance(eth_account_address, None)
    //    .await?;

    //let weth_azero_address_bytes: [u8; 32] = weth_azero_address.into();
    //let send_request_args = [
    //    azero::bytes32_to_string(&weth_azero_address_bytes),
    //    transfer_amount.to_string(),
    //    azero::bytes32_to_string(&eth_account_address_bytes),
    //];

    //let send_request_info = most
    //    .contract_exec_value(
    //        &azero_signed_connection,
    //        "send_request",
    //        &send_request_args,
    //        200_000_000_000_000
    //    )
    //    .await?;
    //info!("`send_request` tx info: {:?}", send_request_info);

    //let wait = tokio::time::Duration::from_secs(5_u64);
    //tokio::time::sleep(wait).await;

    //let balance_post_unwrap = eth_connection
    //    .get_balance(eth_account_address, None)
    //    .await?;

    //assert_eq!(
    //    (balance_post_unwrap - balance_pre_unwrap).as_u128(),
    //    transfer_amount
    //);

    let base_fee = most.contract_read0(&azero_signed_connection, "get_base_fee").await?;
    info!("base_fee: {:?}:", base_fee);

    Ok(())
}
