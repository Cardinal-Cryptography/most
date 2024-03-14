use std::{
    cmp::min,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::ContractError,
    providers::{Http, Middleware, Provider, ProviderError},
    types::BlockNumber,
    utils::keccak256,
};
use log::{debug, error, info, trace, warn};
use redis::{aio::Connection as RedisConnection, RedisError};
use thiserror::Error;
use tokio::{
    sync::{
        broadcast,
        mpsc::{self},
        oneshot, Mutex,
    },
    time::{sleep, Duration},
};

use super::EthMostEvents;
use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::EthConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 15;
// pub const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

pub struct EthListener;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthListenerError {
    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Http>>),

    #[error("redis connection error")]
    Redis(#[from] RedisError),

    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<EthMostEvents>),

    #[error("channel broadcast error")]
    Broadcast(#[from] broadcast::error::SendError<u32>),

    #[error("channel receive error")]
    Receive(#[from] broadcast::error::RecvError),
}

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        eth_connection: Arc<EthConnection>,
        eth_events_sender: mpsc::Sender<EthMostEvents>,
        last_processed_block_number: broadcast::Sender<u32>,
        mut next_unprocessed_block_number: broadcast::Receiver<u32>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            name,
            default_sync_from_block_eth,
            sync_step,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let most_eth = Most::new(address, Arc::clone(&eth_connection));

        loop {
            let unprocessed_block_number = next_unprocessed_block_number.recv().await?;

            // Query for the next unknown finalized block number, if not present we wait for it.
            let next_finalized_block_number = get_next_finalized_block_number_eth(
                eth_connection.clone(),
                unprocessed_block_number,
            )
            .await;

            // don't query for more than `sync_step` blocks at one time.
            let to_block = min(
                next_finalized_block_number,
                unprocessed_block_number + sync_step,
            );

            info!(
                "Processing events from blocks {} - {}",
                unprocessed_block_number, to_block
            );

            // Query for events
            let events = most_eth
                .events()
                .from_block(unprocessed_block_number)
                .to_block(to_block)
                .query()
                .await?;

            let (events_ack_sender, events_ack_receiver) = oneshot::channel();

            info!("Sending a batch of {} events", events.len());

            eth_events_sender
                .send(EthMostEvents {
                    events,
                    events_ack_sender,
                })
                .await?;

            // wait for ack before moving on to the next batch
            info!("Awaiting ack");
            _ = events_ack_receiver.await;

            // publish this block number as processed
            last_processed_block_number.send(unprocessed_block_number)?;
        }
    }
}

pub async fn get_next_finalized_block_number_eth(
    eth_connection: Arc<EthConnection>,
    not_older_than: u32,
) -> u32 {
    loop {
        match eth_connection.get_block(BlockNumber::Finalized).await {
            Ok(block) => match block {
                Some(block) => {
                    let best_finalized_block_number = block
                        .number
                        .expect("Finalized block has a number.")
                        .as_u32();
                    if best_finalized_block_number >= not_older_than {
                        return best_finalized_block_number;
                    }
                }
                None => {
                    warn!("No finalized block found.");
                }
            },
            Err(e) => {
                warn!("Client error when getting last finalized block: {e}");
            }
        };

        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
    }
}

// async fn wait_until_not_halted(
//     most_azero: &MostInstance,
//     azero_connection: &AzeroConnectionWithSigner,
// ) -> Result<(), EthListenerError> {
//     loop {
//         if !most_azero.is_halted(azero_connection).await? {
//             return Ok(());
//         }
//         sleep(Duration::from_secs(10)).await;
//     }
// }