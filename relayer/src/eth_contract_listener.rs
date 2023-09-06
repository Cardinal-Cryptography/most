use crate::config::Config;
use ethers::{
    contract::abigen,
    core::types::Address,
    prelude::ContractError,
    providers::{Provider, ProviderError, StreamExt, Ws},
};
// use eyre::Result;
use std::sync::Arc;
use thiserror::Error;

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

    let provider = Provider::<Ws>::connect(eth_node_wss_url).await?;
    let client = Arc::new(provider);
    let address: Address = eth_contract_address.parse()?;

    let contract: Flipper<Provider<Ws>> = Flipper::new(address, client);

    let events = contract.event::<FlipFilter>().from_block(*eth_from_block);
    let mut stream = events.stream().await?.take(1);

    while let Some(Ok(f)) = stream.next().await {
        println!("Flipper event: {f:?}");
    }

    Ok(())
}
