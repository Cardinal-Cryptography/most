#[derive(Debug, clap::Parser)]
pub struct Config {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub azero_contract_address: String,

    #[arg(long, default_value = "../azero/artifacts/membrane.json")]
    pub azero_contract_metadata: String,

    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    pub azero_node_wss_url: String,

    #[arg(long, default_value = "//Alice")]
    pub azero_sudo_seed: String,

    #[arg(long)]
    pub eth_contract_address: String,

    #[arg(long, default_value = "")]
    pub eth_keystore_password: String,

    #[arg(
        long,
        default_value = "../devnet-eth/execution/keystore/123463a4b065722e99115d6c222f267d9cabb524"
    )]
    pub eth_keystore_path: String,

    #[arg(long, default_value = "ws://127.0.0.1:8546")]
    pub eth_node_wss_url: String,

    #[arg(long, default_value = "10")]
    pub eth_tx_submission_retries: usize,

    #[arg(long, default_value = "32")]
    pub eth_tx_min_confirmations: usize,

    #[arg(long, default_value = "0")]
    pub default_sync_from_block_eth: u32,

    #[arg(long, default_value = "0")]
    pub default_sync_from_block_azero: u32,

    #[arg(long, default_value = "1000")]
    pub sync_step: u32,

    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    pub redis_node: String,

    #[arg(long, default_value = "info")]
    pub rust_log: log::Level,
}
