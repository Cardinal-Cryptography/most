#[derive(Debug, clap::Parser)]
pub struct Config {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub dev: bool,

    #[arg(long)]
    pub azero_contract_address: String,

    #[arg(long, default_value = "../azero/artifacts/most.json")]
    pub azero_contract_metadata: String,

    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    pub azero_node_wss_url: String,

    #[arg(long, default_value = "1000")]
    pub azero_max_event_handler_tasks: usize,

    #[arg(long)]
    pub eth_contract_address: String,

    #[arg(long, default_value = "")]
    pub eth_keystore_password: String,

    #[arg(long, default_value = "0")]
    pub dev_account_index: u32,

    #[arg(long, default_value = "")]
    pub eth_keystore_path: String,

    #[arg(long, default_value = "http://127.0.0.1:8545")]
    pub eth_node_http_url: String,

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

    #[arg(long, default_value = "200000")]
    pub eth_gas_limit: u32,

    #[arg(long, default_value = "100000000000")]
    pub azero_ref_time_limit: u64,

    #[arg(long, default_value = "10000000")]
    pub azero_proof_size_limit: u64,
}
