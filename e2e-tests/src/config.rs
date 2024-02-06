use clap::{Args, Parser};
#[derive(Debug, Parser)]
pub struct Config {
    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    pub azero_node_ws: String,

    #[arg(long, default_value = "../azero/addresses.json")]
    pub azero_contract_addresses_path: String,

    #[arg(long, default_value = "../azero/env/dev.json")]
    pub azero_metadata_path: String,

    #[arg(long, default_value = "http://127.0.0.1:8545")]
    pub eth_node_http: String,

    #[arg(long, default_value = "0")]
    pub eth_dev_account_index: u32,

    #[arg(long, default_value = "../eth/addresses.json")]
    pub eth_contract_addresses_path: String,

    #[command(flatten)]
    pub eth_contract_metadata_paths: EthContractMetadataPaths,

    #[arg(long, default_value = "200000")]
    pub eth_gas_limit: u128,

    #[command(flatten)]
    pub test_args: TestArgs,

    #[arg(long, default_value = "info")]
    pub rust_log: log::Level,
}

#[derive(Args, Debug)]
pub struct EthContractMetadataPaths {
    #[arg(
        long,
        default_value = "../eth/artifacts/contracts/Governance.sol/Governance.json"
    )]
    pub governance: String,
    
    #[arg(
        long,
        default_value = "../eth/artifacts/contracts/Migrations.sol/Migrations.json"
    )]
    pub migrations: String,

    #[arg(long, default_value = "../eth/artifacts/contracts/Most.sol/Most.json")]
    pub most: String,

    #[arg(
        long,
        default_value = "../eth/artifacts/contracts/WETH9.sol/WETH9.json"
    )]
    pub weth9: String,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    #[arg(long, default_value = "1")]
    pub transfer_amount: u128,
}