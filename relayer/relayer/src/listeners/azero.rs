use std::{cmp::min, sync::Arc, time::Duration};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    utility::BlocksApi,
    AlephConfig, AsConnection, Connection,
};
use log::{debug, error, info, warn};
use subxt::events::Events;
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    task::{JoinError, JoinSet},
    time::sleep,
};

use super::AzeroMostEvents;
use crate::{
    config::Config,
    connections::azero::AzeroWsConnection,
    contracts::{AzeroContractError, MostInstance},
    CircuitBreakerEvent,
};

pub const ALEPH_BLOCK_PROD_TIME_SEC: u64 = 1;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroListenerError {
    #[error("Aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("Subxt error")]
    Subxt(#[from] subxt::Error),

    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("No block found")]
    BlockNotFound,

    #[error("Task join error")]
    Join(#[from] JoinError),

    #[error("events channel send error")]
    EventsSend(#[from] mpsc::error::SendError<AzeroMostEvents>),

    #[error("block seal send error")]
    BlockSealSend(#[from] mpsc::error::SendError<u32>),

    #[error("broadcast send error")]
    Broadcast(#[from] broadcast::error::SendError<u32>),

    #[error("broadcast receive error")]
    Receive(#[from] broadcast::error::RecvError),
}

#[derive(Copy, Clone)]
pub struct AlephZeroListener;

impl AlephZeroListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<Connection>,
        azero_events_sender: mpsc::Sender<AzeroMostEvents>,
        last_processed_block_number: broadcast::Sender<u32>,
        mut next_unprocessed_block_number: broadcast::Receiver<u32>,
        block_seal_sender: mpsc::Sender<u32>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AlephZeroListenerError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            azero_ref_time_limit,
            azero_proof_size_limit,
            sync_step,
            ..
        } = &*config;

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        loop {
            debug!(target: "AlephZeroListener", "Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!(target: "AlephZeroListener", "Exiting before handling next block due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                Ok (unprocessed_block_number) = next_unprocessed_block_number.recv() => {

                    // Query for the next unknown finalized block number, if not present we wait for it
                    let next_finalized_block_number = get_next_finalized_block_number_azero(
                        azero_connection.clone(),
                        unprocessed_block_number,
                    )
                        .await;

                    let to_block = min(
                        next_finalized_block_number,
                        unprocessed_block_number + (*sync_step) - 1,
                    );

                    info!(target: "AlephZeroListener",
                          "Processing events from blocks {} - {}",
                          unprocessed_block_number, to_block
                    );

                    // Fetch the events in parallel.
                    let all_events = fetch_events_in_block_range(
                        azero_connection.clone(),
                        unprocessed_block_number,
                        to_block,
                    )
                        .await?;

                    let filtered_events = all_events
                        .into_iter()
                        .flat_map(|(block_details, events)| {
                            most_azero.filter_events(events, block_details.clone())
                        })
                        .collect::<Vec<ContractEvent>>();

                    if filtered_events.is_empty () {

                        select! {
                            cb_event = circuit_breaker_receiver.recv () => {
                                warn!(target: "AlephZeroListener", "Exiting before sending block seal request for {to_block} due to a circuit breaker event {cb_event:?}");
                                return Ok(cb_event?);
                            },

                            Ok (_)= block_seal_sender.send (to_block) => {
                                info!("Marking all events up to block {to_block} as handled");
                            }

                        }

                    } else {

                        let (ack_sender, ack_receiver) = oneshot::channel::<()> ();

                        // there are events to handle
                        select! {
                            cb_event = circuit_breaker_receiver.recv () => {
                                warn!(target: "AlephZeroListener", "Exiting before sending events due to a circuit breaker event {cb_event:?}");
                                return Ok(cb_event?);
                            },

                            Ok (_) = azero_events_sender
                                .send(AzeroMostEvents {
                                    events: filtered_events.clone (),
                                    from_block: unprocessed_block_number,
                                    to_block,
                                    ack: ack_sender
                                }) => {
                                    info!(target: "AlephZeroListener", "Sending a batch of {} events", &filtered_events.len());
                                },
                        }

                        // block until this events batch is handled & notify cache manager
                        select! {
                            cb_event = circuit_breaker_receiver.recv () => {
                                warn!(target: "AlephZeroListener", "Exiting before sealing {to_block} due to a circuit breaker event {cb_event:?}");
                                return Ok(cb_event?);
                            },
                            _ = ack_receiver => {
                                info!("Marking all events up to block {to_block} as handled");
                                block_seal_sender.send (to_block) .await?;
                            }
                        }

                    }

                    info!(target: "AlephZeroListener", "Sending {} as the next unprocessed block number", to_block + 1);
                    last_processed_block_number.send(to_block + 1)?;
                }
            }
        }
    }
}

async fn fetch_events_in_block_range(
    azero_connection: Arc<AzeroWsConnection>,
    from_block: u32,
    to_block: u32,
) -> Result<Vec<(BlockDetails, Events<AlephConfig>)>, AlephZeroListenerError> {
    let mut event_fetching_tasks = JoinSet::new();

    for block_number in from_block..=to_block {
        let azero_connection = azero_connection.clone();

        event_fetching_tasks.spawn(async move {
            let block_hash = azero_connection
                .get_block_hash(block_number)
                .await?
                .ok_or(AlephZeroListenerError::BlockNotFound)?;

            let events = azero_connection
                .as_connection()
                .as_client()
                .blocks()
                .at(block_hash)
                .await?
                .events()
                .await?;

            Ok::<_, AlephZeroListenerError>((
                BlockDetails {
                    block_number,
                    block_hash,
                },
                events,
            ))
        });
    }

    let mut block_events = Vec::new();

    // Wait for all event processing tasks to finish.
    while let Some(result) = event_fetching_tasks.join_next().await {
        block_events.push(result??);
    }

    Ok(block_events)
}

async fn get_next_finalized_block_number_azero(
    azero_connection: Arc<AzeroWsConnection>,
    not_older_than: u32,
) -> u32 {
    loop {
        match azero_connection.get_finalized_block_hash().await {
            Ok(hash) => match azero_connection.get_block_number(hash).await {
                Ok(number_opt) => {
                    let best_finalized_block_number =
                        number_opt.expect("Finalized block has a number.");
                    if best_finalized_block_number >= not_older_than {
                        return best_finalized_block_number;
                    }
                }
                Err(err) => {
                    warn!("Aleph Client error when getting best finalized block number: {err}");
                }
            },
            Err(err) => {
                warn!("Aleph Client error when getting best finalized block hash: {err}");
            }
        };

        // If we are up to date, we can sleep for a longer time.
        sleep(Duration::from_secs(10 * ALEPH_BLOCK_PROD_TIME_SEC)).await;
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroHaltedListenerError {
    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),
}

#[derive(Copy, Clone)]
pub struct AlephZeroHaltedListener;

impl AlephZeroHaltedListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AlephZeroHaltedListenerError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            azero_ref_time_limit,
            azero_proof_size_limit,
            ..
        } = &*config;

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        info!(
            target: "AlephZeroHaltedListener",
            "Starting"
        );

        loop {
            debug!(target: "AlephZeroHaltedListener", "Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!(target: "AlephZeroHaltedListener","Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                is_halted = most_azero.is_halted(&azero_connection) => {
                    debug!(target: "AlephZeroHaltedListener", "Querying");
                    if is_halted? {
                        circuit_breaker_sender.send(CircuitBreakerEvent::BridgeHaltAlephZero)?;
                        warn!(target: "AlephZeroHaltedListener",
                              "Most is halted, exiting");
                        return Ok(CircuitBreakerEvent::BridgeHaltAlephZero);
                    }

                    // sleep before making another query
                    sleep(Duration::from_secs(ALEPH_BLOCK_PROD_TIME_SEC)).await;
                }
            }
        }
    }
}
