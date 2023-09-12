use std::sync::Arc;

use ethers::{
    core::types::Address,
    prelude::ContractError,
    providers::{Middleware, Provider, ProviderError, StreamExt, Ws},
};
use log::info;
use thiserror::Error;

use crate::{
    config::Config,
    connections::{azero::SignedAzeroWsConnection, eth::SignedEthWsConnection},
    contracts::{AzeroContractError, Flipper, FlipperEvents, FlipperInstance},
    helpers::chunks,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthListenerError {
    #[error("provider error")]
    Provider(#[from] ProviderError),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Ws>>),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),
}

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            eth_last_known_block,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Flipper::new(address, Arc::clone(&eth_connection));

        let last_block_number = eth_connection.get_block_number().await.unwrap().as_u32();

        // replay past events from the last known block
        for (from, to) in chunks(*eth_last_known_block as u32, last_block_number, 1000) {
            let past_events = contract
                .events()
                .from_block(from)
                .to_block(to)
                .query()
                .await
                .unwrap();

            for event in past_events {
                handle_event(&event, &config, Arc::clone(&azero_connection)).await?
            }
        }

        info!("finished processing past events");

        // subscribe to new events
        let events = contract.events().from_block(last_block_number);
        let mut stream = events.stream().await.unwrap();

        info!("subscribing to new events");

        while let Some(Ok(event)) = stream.next().await {
            handle_event(&event, &config, Arc::clone(&azero_connection)).await?;
        }

        Ok(())
    }
}

async fn handle_event(
    event: &FlipperEvents,
    config: &Config,
    azero_connection: Arc<SignedAzeroWsConnection>,
) -> Result<(), EthListenerError> {
    if let FlipperEvents::FlipFilter(flip_event) = event {
        let Config {
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        info!("handling eth contract event: {flip_event:?}");

        let contract = FlipperInstance::new(azero_contract_address, azero_contract_metadata)?;

        contract.flop(&azero_connection).await?;
    }

    Ok(())
}
