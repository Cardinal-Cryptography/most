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

pub async fn run(config: Arc<Config>) -> Result<(), EthListenerError> {
    let Config {
        eth_node_wss_url,
        eth_contract_address,
        eth_last_known_block,
        ..
    } = &*config;

    let provider = connect(eth_node_wss_url).await?;
    let client = Arc::new(provider);

    let address = eth_contract_address.parse::<Address>()?;
    let contract = Flipper::new(address, Arc::clone(&client));

    let last_block_number = client.get_block_number().await?.as_u32();

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
            .try_for_each(|event| -> Result<(), EthListenerError> { handle_event(event) })?;
    }

    info!("finished processing past events");

    // subscribe to new events
    let events = contract.events().from_block(last_block_number);
    let mut stream = events.stream().await?;

    info!("subscribing to new events");

    while let Some(Ok(event)) = stream.next().await {
        handle_event(&event)?;
    }

    Ok(())
}

fn handle_event(event: &FlipperEvents) -> Result<(), EthListenerError> {
    if let FlipperEvents::FlipFilter(flip_event) = event {
        info!("handling eth contract event: {flip_event:?}");
    }

    Ok(())
}

async fn connect(url: &str) -> Result<Provider<Ws>, EthListenerError> {
    Ok(Provider::<Ws>::connect(url).await?)
}
