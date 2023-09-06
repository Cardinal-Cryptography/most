use crate::config::Config;
use ethers::{
    contract::abigen,
    core::types::Address,
    prelude::{Contract, ContractError},
    providers::{Provider, ProviderError, StreamExt, Ws},
    types::{BlockNumber, Filter, H160, H256},
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

// 1337..BlockNumber::Latest
pub async fn run(config: Arc<Config>) -> Result<(), EthListenerError> {
    let Config {
        eth_node_wss_url,
        eth_contract_address,
        eth_from_block,
        ..
    } = &*config;

    let provider = connect(eth_node_wss_url).await?;
    let client = Arc::new(provider);

    // let token_topics = vec![H256::from(eth_contract_address.parse::<H160>()?)];

    let address = eth_contract_address.parse::<Address>()?;
    let contract = Flipper::new(address, client);
    let events = contract.events();

    // replay past events
    // TODO : in chunks
    let past_events = events
        .from_block(0)
        .to_block(BlockNumber::Latest)
        .query()
        .await?;
    println!("{:?}", past_events);
    println!("{}", past_events.len());

    // subscribe to new events

    let events = contract.events().from_block(0);
    let mut stream = events.stream().await?.with_meta();
    while let Some(Ok((event, meta))) = stream.next().await {
        println!("{event:?}, {meta:?}");
    }

    Ok(())
}

async fn connect(url: &str) -> Result<Provider<Ws>, EthListenerError> {
    Ok(Provider::<Ws>::connect(url).await?)
}

fn chunks(from: i32, to: i32, step: i32) -> Vec<(i32, i32)> {
    let mut intervals = Vec::new();
    let mut current = from;

    while current < to {
        let next = current + step;
        if next > to {
            intervals.push((current, to));
        } else {
            intervals.push((current, next));
        }
        current = next;
    }

    intervals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunks() {
        let from = 0;
        let to = 21;
        let step = 5;
        let intervals = chunks(from, to, step);
        assert_eq!(
            vec![(0, 5), (5, 10), (10, 15), (15, 20), (20, 21)],
            intervals
        );
    }
}
