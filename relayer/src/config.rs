#[derive(Debug, clap::Parser)]
pub struct Config {
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

    #[arg(long, default_value = "chaos555")]
    pub eth_keystore_password: String,

    #[arg(
        long,
        default_value = "../0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/keystore/f2f0930c3b7bdf1734ee173272bd8cdc0a08f038"
    )]
    pub eth_keystore_path: String,

    #[arg(long, default_value = "ws://127.0.0.1:8546")]
    pub eth_node_wss_url: String,

    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    pub redis_node: String,

    #[arg(long, default_value = "info")]
    pub rust_log: log::Level,
}
