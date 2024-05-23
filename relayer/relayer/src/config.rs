use std::{cmp::max, ops::Deref, str::FromStr};

use ethers::core::types::H256;

#[derive(Debug, Clone)]
pub struct SyncFromBlock(u32);

impl FromStr for SyncFromBlock {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner: u32 = s.parse().map_err(|e| format!("{e}"))?;
        Ok(Self(max(1, inner)))
    }
}

impl Deref for SyncFromBlock {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, clap::Parser)]
pub struct Config {
    #[arg(long)]
    pub name: String,

    #[arg(long)]
    pub dev: bool,

    #[arg(long, default_value = "0")]
    pub dev_account_index: u32,

    #[arg(long)]
    pub override_azero_cache: bool,

    #[arg(long)]
    pub override_eth_cache: bool,

    /// Optional list of hex encoded request hashes to skip from processing
    #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
    pub blacklisted_requests: Option<Vec<H256>>,

    /// Whether to include trading component that makes periodic swaps from A0 -> Ethereum
    #[arg(
        long,
        default_value = "false",
        requires("router_address"),
        requires("azero_wrapped_azero_address"),
        requires("azero_ether_address")
    )]
    pub run_trader_component: bool,

    /// Trader component will always keep this + current base_fee in the guardians balance and will only sell the surplus
    ///
    /// Defaults to 100 AZERO
    #[arg(long, default_value = "100_000_000_000_000")]
    pub eth_to_azero_relaying_buffer: u128,

    /// Trader component will bridge azero wETH after the balance exceeds this amount
    ///
    /// Defaults to 0.1 ETH
    #[arg(long, default_value = "100_000_000_000_000_000")]
    pub bridging_threshold: u128,

    /// Trader component will claim rewards when they exceed this value
    ///
    /// Defaults to 10 AZERO
    #[arg(long, default_value = "10_000_000_000_000")]
    pub reward_withdrawal_threshold: u128,

    #[arg(long)]
    pub router_address: Option<String>,

    #[arg(long, default_value = "../azero/external_artifacts/router.json")]
    pub router_metadata: String,

    /// Ethereum PSP22 token on the AlephZero
    #[arg(long)]
    pub azero_ether_address: Option<String>,

    #[arg(long, default_value = "../azero/artifacts/token.json")]
    pub azero_ether_metadata: String,

    #[arg(long)]
    pub azero_wrapped_azero_address: Option<String>,

    #[arg(long, use_value_delimiter = true, value_delimiter = ',')]
    pub advisory_contract_addresses: Option<Vec<String>>,

    #[arg(long, default_value = "../azero/artifacts/advisory.json")]
    pub advisory_contract_metadata: String,

    #[arg(long)]
    pub signer_cid: Option<u32>,

    #[arg(long, default_value = "1234")]
    pub signer_port: u32,

    #[arg(long)]
    pub azero_contract_address: String,

    #[arg(long, default_value = "../azero/artifacts/most.json")]
    pub azero_contract_metadata: String,

    #[arg(long, default_value = "ws://127.0.0.1:9944")]
    pub azero_node_wss_url: String,

    #[arg(long, default_value = "1000")]
    pub azero_max_event_handler_tasks: usize,

    #[arg(long, default_value = "100000000000")]
    pub azero_ref_time_limit: u64,

    #[arg(long, default_value = "10000000")]
    pub azero_proof_size_limit: u64,

    #[arg(long, default_value = "1")]
    pub default_sync_from_block_azero: SyncFromBlock,

    #[arg(long)]
    pub eth_contract_address: String,

    #[arg(long, default_value = "http://127.0.0.1:8545")]
    pub eth_node_http_url: String,

    #[arg(long, default_value = "10")]
    pub eth_tx_submission_retries: usize,

    #[arg(long, default_value = "32")]
    pub eth_tx_min_confirmations: usize,

    #[arg(long, default_value = "1")]
    pub default_sync_from_block_eth: SyncFromBlock,

    #[arg(long, default_value = "200000")]
    pub eth_gas_limit: u32,

    #[arg(long, default_value = "90")]
    pub eth_poll_interval: u64,

    #[arg(long, default_value = "100")]
    pub sync_step: u32,

    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    pub redis_node: String,

    #[arg(long, default_value = "info")]
    pub rust_log: log::Level,
}
