use std::{cmp::min, sync::Arc};

use ethers::{
    core::types::Address,
    prelude::ContractError,
    providers::{Http, Middleware, Provider},
    types::BlockNumber,
};
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    time::{sleep, Duration},
};

use super::EthMostEvents;
use crate::{
    config::Config, connections::eth::EthConnection, contracts::Most, CircuitBreakerEvent,
};

pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 15;

pub struct EthereumListener;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumListenerError {
    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Http>>),

    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<EthMostEvents>),

    #[error("channel broadcast error")]
    Broadcast(#[from] broadcast::error::SendError<u32>),

    #[error("channel receive error")]
    Receive(#[from] broadcast::error::RecvError),
}

impl EthereumListener {
    pub async fn run(
        config: Arc<Config>,
        eth_connection: Arc<EthConnection>,
        eth_events_sender: mpsc::Sender<EthMostEvents>,
        last_processed_block_number: broadcast::Sender<u32>,
        mut next_unprocessed_block_number: broadcast::Receiver<u32>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, EthereumListenerError> {
        let Config {
            eth_contract_address,
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
                unprocessed_block_number + sync_step - 1,
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

            info!("Awaiting ack");
            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                _ = events_ack_receiver => {
                    info!("Events ack received");
                    // publish this block number as the last fully processed
                    info!("Marking {to_block} as the most recently seen block number");
                    last_processed_block_number.send(to_block + 1)?;
                }
            }
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

        debug!("Waiting for a next finalized block");
        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumPausedListenerError {
    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Http>>),
}

pub struct EthereumPausedListener;

impl EthereumPausedListener {
    pub async fn run(
        config: Arc<Config>,
        eth_connection: Arc<EthConnection>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, EthereumPausedListenerError> {
        let Config {
            eth_contract_address,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let most_eth = Most::new(address, Arc::clone(&eth_connection));

        loop {
            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                else => {
                    if most_eth.paused().await? {
                        circuit_breaker_sender.send(CircuitBreakerEvent::BridgeHaltEthereum)?;
                        return Ok (CircuitBreakerEvent::BridgeHaltEthereum)
                    }
                }
            }
        }
    }
}
