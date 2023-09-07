use std::sync::Arc;

use aleph_client::{
    contract::event::{listen_contract_events, ContractEvent},
    pallets::balances::BalanceUserApi,
    AlephConfig, Connection, ConnectionApi, KeyPair, SignedConnection, TxStatus,
};
use subxt::{blocks::BlocksClient, config::Header, OnlineClient};
use thiserror::Error;

use crate::config::Config;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {
    #[error("provider error")]
    Subxt(#[from] subxt::Error),
}

pub async fn run(config: Arc<Config>) -> Result<(), AzeroListenerError> {
    let Config {
        azero_node_wss_url, ..
    } = &*config;

    println!("@azero listener");

    // TODO : from 0 to latest

    let connection = Connection::new(azero_node_wss_url).await;

    let client: OnlineClient<AlephConfig> = connection.as_client().to_owned();
    let blocks_client = client.blocks();
    let rpc_client = client.rpc();

    // rpc

    let genesis_hash = client.genesis_hash();

    let b = blocks_client.at(genesis_hash).await?;
    let parent_hash = b.header().parent_hash;

    println!("@genesis_hash {genesis_hash} parent {parent_hash}");

    let b = blocks_client.at_latest().await?;
    let hash = b.header().hash();
    let parent_hash = b.header().parent_hash;

    println!("@ hash: {hash} parent_hash: {parent_hash}");

    // let subscription = connection
    //     .as_client()
    //     .blocks()
    //     .subscribe_finalized()
    //     .await?;

    Ok(())
}
