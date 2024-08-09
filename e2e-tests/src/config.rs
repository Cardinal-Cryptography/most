use std::{env, str::FromStr};

use aleph_client::sp_runtime::AccountId32;
use anyhow::{anyhow, Result};
use ethers::types::Address;
use once_cell::sync::Lazy;

use crate::{azero, eth};

static GLOBAL_CONFIG: Lazy<Config> = Lazy::new(|| Config {
    azero_node_ws: get_env("AZERO_NODE_WS").unwrap_or("ws://127.0.0.1:9944".to_string()),
    azero_contract_addresses_path: get_env("AZERO_CONTRACT_ADDRESSES_PATH")
        .unwrap_or("../azero/addresses.json".to_string()),
    azero_account_seed: get_env("AZERO_ACCOUNT_SEED").unwrap_or("//Alice".to_string()),
    eth_node_http: get_env("ETH_NODE_HTTP").unwrap_or("http://127.0.0.1:8545".to_string()),
    eth_dev_account_index: get_env("ETH_DEV_ACCOUNT_INDEX").unwrap_or(0),
    eth_contract_addresses_path: get_env("ETH_CONTRACT_ADDRESSES_PATH")
        .unwrap_or("../eth/addresses.json".to_string()),
    contract_metadata_paths: ContractMetadataPaths {
        azero_most: get_env("AZERO_MOST").unwrap_or("../azero/artifacts/most.json".to_string()),
        azero_token: get_env("AZERO_TOKEN").unwrap_or("../azero/artifacts/token.json".to_string()),
        azero_wazero: get_env("AZERO_WAZERO")
            .unwrap_or("../azero/artifacts/wrapped_azero.json".to_string()),
        eth_most: get_env("ETH_MOST")
            .unwrap_or("../eth/artifacts/contracts/Most.sol/Most.json".to_string()),
        eth_weth: get_env("ETH_WETH9")
            .unwrap_or("../eth/artifacts/contracts/WETH9.sol/WETH9.json".to_string()),
        eth_usdt: get_env("ETH_USDT")
            .unwrap_or("../eth/artifacts/contracts/USDT.sol/TetherToken.json".to_string()),
        eth_wrapped_token: get_env("ETH_WRAPPED_TOKEN")
            .unwrap_or("../eth/artifacts/contracts/WrappedToken.sol/WrappedToken.json".to_string()),
    },
    eth_mnemonic: get_env("ETH_MNEMONIC").unwrap_or(
        "harsh master island dirt equip search awesome double turn crush wool grant".to_string(),
    ),
    test_args: TestArgs {
        wait_max_minutes: get_env("WAIT_MAX_MINUTES").unwrap_or(15),
        transfer_amount: get_env("TRANSFER_AMOUNT").unwrap_or("0.1".to_string()),
    },
});

fn get_env<T>(name: &str) -> Option<T>
where
    T: FromStr,
    T::Err: std::fmt::Debug,
{
    env::var(name).ok().map(|v| {
        v.parse()
            .unwrap_or_else(|_| panic!("Failed to parse env var {name}"))
    })
}

pub fn setup_test() -> &'static Config {
    env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Info)
        .filter_module("aleph-client", log::LevelFilter::Warn)
        .init();
    &GLOBAL_CONFIG
}

pub struct Config {
    pub azero_node_ws: String,
    pub azero_contract_addresses_path: String,
    pub azero_account_seed: String,
    pub eth_node_http: String,
    pub eth_dev_account_index: u32,
    pub eth_contract_addresses_path: String,
    pub contract_metadata_paths: ContractMetadataPaths,
    pub eth_mnemonic: String,
    pub test_args: TestArgs,
}

pub struct TestContext {
    pub azero_signed_connection: aleph_client::SignedConnection,
    pub eth_signed_connection: eth::SignedConnection,
    pub most_azero: azero::ContractInstance,
    pub weth_azero: azero::ContractInstance,
    pub usdt_azero: azero::ContractInstance,
    pub wazero_azero: azero::ContractInstance,
    pub most_eth: eth::ContractInstance,
    pub weth_eth: eth::ContractInstance,
    pub usdt_eth: eth::ContractInstance,
    pub wazero_eth: eth::ContractInstance,
}

impl Config {
    pub async fn create_test_context(&self) -> Result<TestContext> {
        let azero_keypair = aleph_client::KeyPair::from_str(&self.azero_account_seed)?;
        let azero_signed_connection =
            aleph_client::SignedConnection::new(&self.azero_node_ws, azero_keypair).await;
        let eth_signed_connection = eth::create_signed_connection(self).await?;

        let azero_contract_addresses =
            azero::contract_addresses(&self.azero_contract_addresses_path)?;
        let eth_contract_addresses = eth::contract_addresses(&self.eth_contract_addresses_path)?;

        let most_azero_address = AccountId32::from_str(&azero_contract_addresses.most)
            .map_err(|e| anyhow!("Cannot parse account id from string: {:?}", e))?;
        let weth_azero_address = AccountId32::from_str(&azero_contract_addresses.weth)
            .map_err(|e| anyhow!("Cannot parse account id from string: {:?}", e))?;
        let usdt_azero_address = AccountId32::from_str(&azero_contract_addresses.usdt)
            .map_err(|e| anyhow!("Cannot parse account id from string: {:?}", e))?;
        let wazero_azero_address = AccountId32::from_str(&azero_contract_addresses.wazero)
            .map_err(|e| anyhow!("Cannot parse account id from string: {:?}", e))?;

        let most_eth_address = eth_contract_addresses.most.parse::<Address>()?;
        let weth_eth_address = eth_contract_addresses.weth.parse::<Address>()?;
        let usdt_eth_address = eth_contract_addresses.usdt.parse::<Address>()?;
        let wazero_eth_address = eth_contract_addresses.wazero.parse::<Address>()?;

        let most_azero = azero::ContractInstance::new(
            most_azero_address,
            &self.contract_metadata_paths.azero_most,
        )?;
        let weth_azero = azero::ContractInstance::new(
            weth_azero_address,
            &self.contract_metadata_paths.azero_token,
        )?;
        let usdt_azero = azero::ContractInstance::new(
            usdt_azero_address,
            &self.contract_metadata_paths.azero_token,
        )?;
        let wazero_azero = azero::ContractInstance::new(
            wazero_azero_address,
            &self.contract_metadata_paths.azero_wazero,
        )?;

        let most_abi = eth::contract_abi(&self.contract_metadata_paths.eth_most)?;
        let most_eth =
            eth::contract_from_deployed(most_eth_address, most_abi, &eth_signed_connection)?;

        let weth_abi = eth::contract_abi(&self.contract_metadata_paths.eth_weth)?;
        let weth_eth =
            eth::contract_from_deployed(weth_eth_address, weth_abi, &eth_signed_connection)?;

        let usdt_abi = eth::contract_abi(&self.contract_metadata_paths.eth_usdt)?;
        let usdt_eth =
            eth::contract_from_deployed(usdt_eth_address, usdt_abi, &eth_signed_connection)?;

        let wazero_abi = eth::contract_abi(&self.contract_metadata_paths.eth_wrapped_token)?;
        let wazero_eth =
            eth::contract_from_deployed(wazero_eth_address, wazero_abi, &eth_signed_connection)?;

        let test_context = TestContext {
            azero_signed_connection,
            eth_signed_connection,
            most_azero,
            weth_azero,
            usdt_azero,
            wazero_azero,
            most_eth,
            weth_eth,
            usdt_eth,
            wazero_eth,
        };
        Ok(test_context)
    }
}

pub struct ContractMetadataPaths {
    pub azero_most: String,
    pub azero_token: String,
    pub azero_wazero: String,
    pub eth_most: String,
    pub eth_weth: String,
    pub eth_usdt: String,
    pub eth_wrapped_token: String,
}

pub struct TestArgs {
    pub wait_max_minutes: u64,
    pub transfer_amount: String,
}
