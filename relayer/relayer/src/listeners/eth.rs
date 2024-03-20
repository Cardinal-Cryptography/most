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
    // #[error("sender dropped before ack was received")]
    // AckSenderDropped,
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

        info!(target: "EthereumListener", "Starting");

        loop {
            debug!(target: "EthereumListener", "Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!(target: "EthereumListener","Exiting before handling next block due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                Ok (unprocessed_block_number) = next_unprocessed_block_number.recv() => {
                    // Query for the next unknown finalized block number, if not present we wait for it.
                    info!(target: "EthereumListener","Waiting for the next finalized block number");

                    select! {
                        cb_event = circuit_breaker_receiver.recv () => {
                            warn!(target: "EthereumListener","Exiting before sending events due to a circuit breaker event {cb_event:?}");
                            return Ok(cb_event?);
                        },

                        next_finalized_block_number = get_next_finalized_block_number_eth(
                            eth_connection.clone(),
                            unprocessed_block_number,
                        ) => {

                            // don't query for more than `sync_step` blocks at one time.
                            let to_block = min(
                                next_finalized_block_number,
                                unprocessed_block_number + sync_step - 1,
                            );

                            info!(target: "EthereumListener",
                                  "Processing events from blocks {} - {}",
                                  unprocessed_block_number, to_block
                            );

                            // listen to events
                            let query = most_eth
                                .events()
                                .from_block(unprocessed_block_number)
                                .to_block(to_block);

                            select! {
                                cb_event = circuit_breaker_receiver.recv () => {
                                    warn!(target: "EthereumListener","Exiting before sending events due to a circuit breaker event {cb_event:?}");
                                    return Ok(cb_event?);
                                },

                                Ok (events) = query.query() => {
                                    if !events.is_empty () {
                                        let (events_ack_sender, events_ack_receiver) = oneshot::channel::<()>();
                                        info!(target: "EthereumListener","Sending a batch of {} events", &events.len());

                                        eth_events_sender
                                            .send(EthMostEvents {
                                                events: events.clone (),
                                                events_ack_sender,
                                            }).await?;

                                        info!(target: "EthereumListener", "Awaiting events ack");

                                        // select between ack and the channel, because the handler could have exited
                                        select! {
                                            cb_event = circuit_breaker_receiver.recv () => {
                                                warn!(target: "EthereumListener", "Exiting before events ack due to a circuit breaker event {cb_event:?}");
                                                return Ok(cb_event?);
                                            },
                                            ack_result = events_ack_receiver => {
                                                // ack_result.map_err (|_| EthereumListenerError::AckSenderDropped)?;
                                                if ack_result.is_ok () {
                                                    info!(target: "EthereumListener", "Events ack received, marking {to_block} as the most recently seen block number");
                                                    // we send + 1 as this is the next block we'd like to see
                                                    last_processed_block_number.send(to_block + 1)?;
                                                }
                                            }
                                        }
                                        // events_ack_receiver.await.map_err (|_| EthereumListenerError::AckSenderDropped)?;
                                    } else {
                                        info!(target: "EthereumListener", "Marking {to_block} as the most recently seen block number");
                                        // we send + 1 as this is the next block we'd like to see
                                        last_processed_block_number.send(to_block + 1)?;
                                    }
                                    // publish this block number as the last fully processed
                                    // info!(target: "EthereumListener","Events ack received, marking {to_block} as the most recently seen block number");
                                    // last_processed_block_number.send(to_block + 1)?;

                                }
                            }
                        }
                    }
                },
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

    #[error("unexpected error")]
    Unexpected,
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

        info!(target: "EthereumPausedListener", "Starting");

        let address = eth_contract_address.parse::<Address>()?;
        let most_eth = Most::new(address, Arc::clone(&eth_connection));

        select! {
            cb_event = circuit_breaker_receiver.recv () => {
                warn!(target: "EthereumPausedListener", "Exiting due to a circuit breaker event {cb_event:?}");
                Ok(cb_event?)
            },

            _ = async {
                loop {
                    debug!(target: "EthereumPausedListener", "Querying");
                    if most_eth.paused().await? {
                        circuit_breaker_sender.send(CircuitBreakerEvent::BridgeHaltEthereum)?;
                        warn!(target: "EthereumPausedListener", "Most is paused, exiting");
                        return Ok::<CircuitBreakerEvent, EthereumPausedListenerError>(CircuitBreakerEvent::BridgeHaltEthereum);
                    }

                    sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
                }
            } => {
                Err (EthereumPausedListenerError::Unexpected)
            }
        }
    }
}
