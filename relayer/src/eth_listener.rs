use std::sync::Arc;

use ethers::{
    contract::abigen,
    core::types::Address,
    prelude::ContractError,
    providers::{Middleware, Provider, ProviderError, StreamExt, Ws},
};
use thiserror::Error;

use crate::{config::Config, helpers::chunks};

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

abigen!(
    Flipper,
    r#"[
        event Flip(bool newValue)
    ]"#,
);

pub async fn run(config: Arc<Config>) -> Result<(), EthListenerError> {
    let Config {
        eth_node_wss_url,
        eth_contract_address,
        eth_from_block,
        ..
    } = &*config;

    let provider = connect(eth_node_wss_url).await?;
    let client = Arc::new(provider);

    let address = eth_contract_address.parse::<Address>()?;
    let contract = Flipper::new(address, Arc::clone(&client));

    let last_block_number = client.get_block_number().await?.as_u32();

    // replay past events
    for (from, to) in chunks(*eth_from_block as u32, last_block_number, 1000) {
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

    // subscribe to new events
    let events = contract.events().from_block(*eth_from_block);
    let mut stream = events.stream().await?;
    while let Some(Ok(event)) = stream.next().await {
        handle_event(&event)?;
    }

    Ok(())
}

fn handle_event(event: &FlipFilter) -> Result<(), EthListenerError> {
    println!("handling event: {event:?}");
    Ok(())
}

async fn connect(url: &str) -> Result<Provider<Ws>, EthListenerError> {
    Ok(Provider::<Ws>::connect(url).await?)
}
