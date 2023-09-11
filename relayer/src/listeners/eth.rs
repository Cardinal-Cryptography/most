use std::sync::{Arc, Mutex};

use ethers::{
    core::types::Address,
    prelude::ContractError,
    providers::{Middleware, Provider, ProviderError, StreamExt, Ws},
};
use log::info;
use thiserror::Error;

use crate::{
    azero_contracts::FlipperInstance,
    config::Config,
    connections::{azero::sign, AzeroWsConnection, EthWsConnection},
    eth_contracts::{Flipper, FlipperEvents},
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
}

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: AzeroWsConnection,
        eth_connection: EthWsConnection,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            eth_last_known_block,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Flipper::new(address, Arc::clone(&eth_connection));

        let last_block_number = eth_connection.get_block_number().await?.as_u32();

        // replay past events from the last known block
        for (from, to) in chunks(*eth_last_known_block as u32, last_block_number, 1000) {
            let past_events = contract
                .events()
                .from_block(from)
                .to_block(to)
                .query()
                .await?;

            past_events
                .iter()
                .try_for_each(|event| -> Result<(), EthListenerError> {
                    handle_event(event, &config, Arc::clone(&azero_connection))
                })?;
        }

        info!("finished processing past events");

        // subscribe to new events
        let events = contract.events().from_block(last_block_number);
        let mut stream = events.stream().await?;

        info!("subscribing to new events");

        while let Some(Ok(event)) = stream.next().await {
            handle_event(&event, &config, Arc::clone(&azero_connection))?;
        }

        Ok(())
    }
}

fn handle_event(
    event: &FlipperEvents,
    config: &Config,
    azero_connection: AzeroWsConnection,
) -> Result<(), EthListenerError> {
    if let FlipperEvents::FlipFilter(flip_event) = event {
        let Config {
            azero_sudo_seed,
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        info!("handling eth contract event: {flip_event:?}");

        let authority = aleph_client::keypair_from_string(azero_sudo_seed);
        let signed_connection = sign(azero_connection, &authority);
        let contract = FlipperInstance::new(&azero_contract_address, &azero_contract_metadata);

        // TODO : send tx
    }

    Ok(())
}
