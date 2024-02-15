use std::{cmp::max, ops::Deref, str::FromStr};

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

    #[arg(long)]
    pub override_azero_cache: bool,

    #[arg(long)]
    pub override_eth_cache: bool,

    #[arg(long, default_value = "0")]
    pub dev_account_index: u32,

    #[arg(long, default_value = "0")]
    pub relayers_committee_id: u128,

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

    #[arg(long)]
    pub eth_contract_address: String,

    #[arg(long, default_value = "")]
    pub eth_keystore_password: String,

    #[arg(long, default_value = "")]
    pub eth_keystore_path: String,

    #[arg(long, default_value = "http://127.0.0.1:8545")]
    pub eth_node_http_url: String,

    #[arg(long, default_value = "10")]
    pub eth_tx_submission_retries: usize,

    #[arg(long, default_value = "32")]
    pub eth_tx_min_confirmations: usize,

    #[arg(long, default_value = "1")]
    pub default_sync_from_block_eth: SyncFromBlock,

    #[arg(long, default_value = "1")]
    pub default_sync_from_block_azero: SyncFromBlock,

    #[arg(long, default_value = "100")]
    pub sync_step: u32,

    #[arg(long, default_value = "redis://127.0.0.1:6379")]
    pub redis_node: String,

    #[arg(long, default_value = "200000")]
    pub eth_gas_limit: u32,

    #[arg(long, default_value = "100000000000")]
    pub azero_ref_time_limit: u64,

    #[arg(long, default_value = "10000000")]
    pub azero_proof_size_limit: u64,
}
