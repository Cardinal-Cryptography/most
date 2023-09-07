use std::sync::Arc;

use aleph_client::{
    contract::{
        event::{listen_contract_events, translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    pallets::balances::BalanceUserApi,
    utility::BlocksApi,
    AlephConfig, Connection, ConnectionApi, KeyPair, SignedConnection, TxStatus,
};
use subxt::{blocks::BlocksClient, config::Header, OnlineClient};
use thiserror::Error;

use crate::{
    aleph_zero::api::contracts::events::ContractEmitted,
    config::Config,
    contracts::{ContractsError, FlipperInstance},
    eth_listener::Flipper,
    helpers::chunks,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("provider error")]
    Subxt(#[from] subxt::Error),

    #[error("contract error")]
    Contracts(#[from] ContractsError),

    #[error("no block found")]
    BlockNotFound,
}

pub async fn run(config: Arc<Config>) -> Result<(), AzeroListenerError> {
    let Config {
        azero_node_wss_url,
        azero_last_known_block,
        azero_contract_metadata,
        azero_contract_address,
        ..
    } = &*config;

    // TODO : from 0 to latest
    let connection = Connection::new(azero_node_wss_url).await;
    let last_block_number = connection
        .get_block_number_opt(None)
        .await?
        .ok_or(AzeroListenerError::BlockNotFound)?;

    let instance = FlipperInstance::new(azero_contract_address, azero_contract_metadata)?;
    let contracts = vec![&instance.contract];

    for (from, to) in chunks(*azero_last_known_block as u32, last_block_number, 1000) {
        for block_number in from..to {
            let block_hash = connection
                .get_block_hash(block_number)
                .await?
                .ok_or(AzeroListenerError::BlockNotFound)?;

            let events = connection
                .as_client()
                .blocks()
                .at(block_hash)
                .await?
                .events()
                .await?;

            // TODO : filter contract events
            for event in translate_events(
                events.iter(),
                &contracts,
                Some(BlockDetails {
                    block_number,
                    block_hash,
                }),
            ) {
                println!("event: {event:?}");
            }
        }
    }

    // let subscription = connection
    //     .as_client()
    //     .blocks()
    //     .subscribe_finalized()
    //     .await?;

    Ok(())
}
