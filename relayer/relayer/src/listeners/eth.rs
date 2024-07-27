use std::{cmp::min, sync::Arc};

use ethers::{
    core::types::Address,
    prelude::ContractError,
    providers::{Http, Provider},
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
    config::Config,
    connections::eth::{get_next_finalized_block_number, EthConnection},
    contracts::Most,
    CircuitBreakerEvent,
};

pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 12;
const LOG_TARGET: &str = "EthereumListener";

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

        info!(target: LOG_TARGET, "Starting");

        loop {
            debug!(target: LOG_TARGET, "Ping");

            let unprocessed_block_number = select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!(target: LOG_TARGET,"Exiting before handling next block due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },
                Ok(unprocessed_block_number) = next_unprocessed_block_number.recv() => {
                    unprocessed_block_number
                }
            };

            // Query for the next unknown finalized block number, if not present we wait for it.
            info!(target: LOG_TARGET, "Waiting for the next finalized block number");

            let next_finalized_block_number = select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!(target: LOG_TARGET, "Exiting before sending events due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },
                next_finalized_block_number = get_next_finalized_block_number(
                    eth_connection.clone(),
                    unprocessed_block_number,
                ) => {
                    next_finalized_block_number
                }
            };

            // don't query for more than `sync_step` blocks at one time.
            let to_block = min(
                next_finalized_block_number,
                unprocessed_block_number + sync_step - 1,
            );

            info!(target: LOG_TARGET,
                  "Processing events from blocks {} - {}",
                  unprocessed_block_number, to_block
            );

            // listen to events
            let query = most_eth
                .events()
                .from_block(unprocessed_block_number)
                .to_block(to_block);

            let events = select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!(target: LOG_TARGET, "Exiting before sending events due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },
                Ok(events) = query.query() => {
                    events
                }
            };

            if events.is_empty() {
                info!(target: LOG_TARGET, "Marking {} as the next unprocessed block number", to_block +1);
                // we send + 1 to self as this is the next block we'd like to see
                last_processed_block_number.send(to_block + 1)?;
                continue;
            }
            let (events_ack_sender, events_ack_receiver) = oneshot::channel::<()>();
            info!(target: LOG_TARGET, "Sending a batch of {} events", &events.len());

            eth_events_sender
                .send(EthMostEvents {
                    events: events.clone(),
                    from_block: unprocessed_block_number,
                    to_block,
                    events_ack_sender,
                })
                .await?;

            info!(target: LOG_TARGET, "Awaiting events ack");

            // select between ack and the channel, because the handler could have exited
            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!(target: LOG_TARGET, "Exiting before events ack due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },
                ack_result = events_ack_receiver => {
                    if ack_result.is_ok () {
                        info!(target: LOG_TARGET, "Events ack received, marking {} as the next unprocessed block number", to_block + 1);
                        // we send + 1 to self as this is the next block we'd like to see
                        last_processed_block_number.send(to_block + 1)?;
                    }
                }
            }
        }
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

        info!(target: "EthereumPausedListener", "Starting");

        let address = eth_contract_address.parse::<Address>()?;
        let most_eth = Most::new(address, Arc::clone(&eth_connection));

        loop {
            debug!(target: "EthereumPausedListener", "Ping");

            let is_paused_call = most_eth.paused();

            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!(target: "EthereumPausedListener","Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                is_paused = is_paused_call.call() => {
                    debug!(target: "EthereumPausedListener", "Querying");
                    match is_paused {
                        Ok(is_paused) => {
                            if is_paused {
                                circuit_breaker_sender.send(CircuitBreakerEvent::BridgeHaltEthereum)?;
                                warn!(target: "EthereumPausedListener",
                                      "Most is paused, exiting");
                                return Ok(CircuitBreakerEvent::BridgeHaltEthereum);
                            }
                        },

                        Err(why) => {
                            warn!("Exiting due to a connection error {why:?}");
                            let status = CircuitBreakerEvent::EthConnectionError;
                            circuit_breaker_sender.send(status.clone())?;
                            return Ok(status.clone());
                        }
                    }

                    // sleep before making another query
                    sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
                }
            }
        }
    }
}
