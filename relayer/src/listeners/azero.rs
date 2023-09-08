use std::sync::Arc;

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    utility::BlocksApi,
    AlephConfig, Connection,
};
use futures::StreamExt;
use log::info;
use subxt::{events::Events, utils::H256};
use thiserror::Error;

use crate::{
    azero_contracts::{ContractsError, FlipperInstance},
    config::Config,
    connections::{AzeroWsConnection, EthWsConnection},
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

pub struct AzeroListener;

impl AzeroListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: AzeroWsConnection,
        eth_connection: EthWsConnection,
    ) -> Result<(), AzeroListenerError> {
        let Config {
            azero_last_known_block,
            azero_contract_metadata,
            azero_contract_address,
            ..
        } = &*config;

        // replay past events from last known to the latest
        let last_block_number = azero_connection
            .get_block_number_opt(None)
            .await?
            .ok_or(AzeroListenerError::BlockNotFound)?;

        let instance = FlipperInstance::new(azero_contract_address, azero_contract_metadata)?;
        let contracts = vec![&instance.contract];

        for (from, to) in chunks(*azero_last_known_block as u32, last_block_number, 1000) {
            for block_number in from..to {
                let block_hash = azero_connection
                    .get_block_hash(block_number)
                    .await?
                    .ok_or(AzeroListenerError::BlockNotFound)?;

                let events = azero_connection
                    .as_client()
                    .blocks()
                    .at(block_hash)
                    .await?
                    .events()
                    .await?;

                // filter contract events
                handle_events(events, &contracts, block_number, block_hash)?;
            }
        }

        info!("finished processing past events");

        // subscribe to new events
        let mut subscription = azero_connection
            .as_client()
            .blocks()
            .subscribe_finalized()
            .await?;

        info!("subscribing to new events");

        while let Some(Ok(block)) = subscription.next().await {
            let events = block.events().await?;
            handle_events(events, &contracts, block.number(), block.hash())?;
        }

        Ok(())
    }
}

fn handle_events(
    events: Events<AlephConfig>,
    contracts: &[&ContractInstance],
    block_number: u32,
    block_hash: H256,
) -> Result<(), AzeroListenerError> {
    for event in translate_events(
        events.iter(),
        contracts,
        Some(BlockDetails {
            block_number,
            block_hash,
        }),
    ) {
        handle_event(event?)?;
    }
    Ok(())
}

fn handle_event(event: ContractEvent) -> Result<(), AzeroListenerError> {
    info!("handling A0 contract event: {event:?}");
    Ok(())
}
