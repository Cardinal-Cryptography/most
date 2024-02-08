use std::{env, str::FromStr, string::ToString};

use once_cell::sync::Lazy;

static GLOBAL_CONFIG: Lazy<Config> = Lazy::new(|| Config {
    azero_node_ws: get_env("AZERO_NODE_WS").unwrap_or("ws://127.0.0.1:9944".to_string()),
    azero_contract_addresses_path: get_env("AZERO_CONTRACT_ADDRESSES_PATH")
        .unwrap_or("../azero/addresses.json".to_string()),
    azero_metadata_path: get_env("AZERO_METADATA_PATH")
        .unwrap_or("../azero/env/dev.json".to_string()),
    azero_account_seed: get_env("AZERO_ACCOUNT_SEED")
        .unwrap_or("Alice".to_string()),
    eth_node_http: get_env("ETH_NODE_HTTP").unwrap_or("http://127.0.0.1:8545".to_string()),
    eth_dev_account_index: get_env("ETH_DEV_ACOUNT_INDEX").unwrap_or(0),
    eth_contract_addresses_path: get_env("ETH_CONTRACT_ADDRESSES_PATH")
        .unwrap_or("../eth/addresses.json".to_string()),
    eth_gas_limit: get_env("ETH_GAS_LIMIT").unwrap_or(200_000),
    contract_metadata_paths: ContractMetadataPaths {
        azero_governance: get_env("AZERO_GOVERNANCE")
            .unwrap_or("../azero/artifacts/governance.json".to_string()),
        azero_migrations: get_env("AZERO_MIGRATIONS")
            .unwrap_or("../azero/artifacts/migrations.json".to_string()),
        azero_most: get_env("AZERO_MOST").unwrap_or("../azero/artifacts/most.json".to_string()),
        azero_oracle: get_env("AZERO_ORACLE")
            .unwrap_or("../azero/artifacts/oracle.json".to_string()),
        azero_token: get_env("AZERO_TOKEN").unwrap_or("../azero/artifacts/token.json".to_string()),
        eth_governance: get_env("")
            .unwrap_or("../eth/artifacts/contracts/Governance.sol/Governance.json".to_string()),
        eth_migrations: get_env("ETH_MIGRATIONS")
            .unwrap_or("../eth/artifacts/contracts/Migrations.sol/Migrations.json".to_string()),
        eth_most: get_env("ETH_MOST")
            .unwrap_or("../eth/artifacts/contracts/Most.sol/Most.json".to_string()),
        eth_weth9: get_env("ETH_WETH9")
            .unwrap_or("../eth/artifacts/contracts/WETH9.sol/WETH9.json".to_string()),
    },
    eth_mnemonic: get_env("ETH_MNEMONIC").unwrap_or(
        "harsh master island dirt equip search awesome double turn crush wool grant".to_string(),
    ),
    test_args: TestArgs {
        wait_max_minutes: get_env("WAIT_MAX_MINUTES").unwrap_or(15),
        transfer_amount: get_env("TRANSFER_AMOUNT").unwrap_or(1),
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
    simple_logger::init_with_env().unwrap();
    &GLOBAL_CONFIG
}

pub struct Config {
    pub azero_node_ws: String,
    pub azero_contract_addresses_path: String,
    pub azero_metadata_path: String,
    pub azero_account_seed: String,
    pub eth_node_http: String,
    pub eth_dev_account_index: u32,
    pub eth_contract_addresses_path: String,
    pub eth_gas_limit: u128,
    pub contract_metadata_paths: ContractMetadataPaths,
    pub eth_mnemonic: String,
    pub test_args: TestArgs,
}

pub struct ContractMetadataPaths {
    pub azero_governance: String,
    pub azero_migrations: String,
    pub azero_most: String,
    pub azero_oracle: String,
    pub azero_token: String,
    pub eth_governance: String,
    pub eth_migrations: String,
    pub eth_most: String,
    pub eth_weth9: String,
}

pub struct TestArgs {
    pub wait_max_minutes: u64,
    pub transfer_amount: u128,
}
