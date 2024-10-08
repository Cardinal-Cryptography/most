use std::{cmp::min, sync::Arc, time::Duration};

use contracts_azero_client::{Client, ContractEvent, ContractInstance};
use futures::{
    future::join_all,
    stream::{FuturesOrdered, StreamExt},
};
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    task::JoinError,
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

    #[error("AzeroClient error")]
    AzeroClient(#[from] contracts_azero_client::ClientError),

    #[error("Subxt error")]
    Subxt(#[from] subxt::Error),

    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Task join error")]
    Join(#[from] JoinError),

    #[error("events channel send error")]
    EventsSend(#[from] mpsc::error::SendError<AzeroMostEvents>),

    #[error("block seal send error")]
    BlockSealSend(#[from] mpsc::error::SendError<u32>),

    #[error("broadcast send error")]
    Broadcast(#[from] broadcast::error::SendError<u32>),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("broadcast receive error")]
    Receive(#[from] broadcast::error::RecvError),

    #[error("One-shot receive error")]
    OneShotReceive(#[from] oneshot::error::RecvError),
}

#[derive(Copy, Clone)]
pub struct AlephZeroListener;

impl AlephZeroListener {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<Client>,
        azero_events_sender: mpsc::Sender<AzeroMostEvents>,
        next_block_to_process_sender: broadcast::Sender<u32>,
        mut next_block_to_process_receiver: broadcast::Receiver<u32>,
        block_seal_sender: mpsc::Sender<u32>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
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

        let mut event_batch_ack_receiver = FuturesOrdered::new();

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

                Ok (unprocessed_block_number) = next_block_to_process_receiver.recv() => {
                    // Query for the next unknown finalized block number, if not present we wait for it
                    let next_finalized_block_number = match get_next_finalized_block_number_azero(
                        azero_connection.clone(),
                        unprocessed_block_number,
                    )
                        .await {
                            Ok(number) => number,
                            Err(AlephZeroListenerError::AlephClient(_)) => {
                                warn!("Aleph client failed when getting next finalized block number. Opening circuit breaker");
                                let status = CircuitBreakerEvent::AlephClientError;
                                circuit_breaker_sender.send(status.clone ())?;
                                return Ok (status);
                            },
                            Err (other) => {
                                return Err (other);
                            }
                        };

                    let to_block = min(
                        next_finalized_block_number,
                        unprocessed_block_number + sync_step - 1,
                    );

                    info!(target: "AlephZeroListener",
                          "Processing events from blocks {} - {}",
                          unprocessed_block_number, to_block
                    );

                    let events = fetch_events_in_block_range(&azero_connection, unprocessed_block_number, to_block, &[&most_azero.contract]).await?;
                    // Fetch the events in parallel.


                    let (ack_sender, ack_receiver) = oneshot::channel::<u32>();
                    event_batch_ack_receiver.push_back(ack_receiver);

                    select! {
                        cb_event = circuit_breaker_receiver.recv() => {
                            warn!(target: "AlephZeroListener", "Exiting before sending events due to a circuit breaker event {cb_event:?}");
                            return Ok(cb_event?);
                        },

                        Ok(_) = azero_events_sender
                            .send(AzeroMostEvents {
                                events: events.clone(),
                                from_block: unprocessed_block_number,
                                to_block,
                                ack: ack_sender
                            }) => {
                                info!(target: "AlephZeroListener", "Sending a batch of {} events", &events.len());
                            },
                    }

                    info!(target: "AlephZeroListener", "Sending {} as the next unprocessed block number", to_block + 1);
                    next_block_to_process_sender.send(to_block + 1)?;
                },
                Some(processed_block_res) = event_batch_ack_receiver.next() => {
                    let processed_block = processed_block_res?;
                    info!("Marking all events up to block {processed_block} as handled");
                    block_seal_sender.send(processed_block).await?;
                }
            }
        }
    }
}

async fn fetch_events_in_block_range(
    azero_connection: &Arc<Client>,
    from_block: u32,
    to_block: u32,
    contracts: &[&ContractInstance],
) -> Result<Vec<ContractEvent>, AlephZeroListenerError> {
    let mut event_fetching_tasks = Vec::new();

    for block_number in from_block..=to_block {
        let azero_connection = azero_connection.clone();

        event_fetching_tasks.push(async move {
            azero_connection
                .fetch_events_from_contracts(block_number, contracts)
                .await
        });
    }

    let events = join_all(event_fetching_tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect();

    Ok(events)
}

async fn get_next_finalized_block_number_azero(
    azero_connection: Arc<AzeroWsConnection>,
    not_older_than: u32,
) -> Result<u32, AlephZeroListenerError> {
    loop {
        let hash = azero_connection.get_finalized_block_hash().await?;
        let best_finalized_block_number = azero_connection
            .get_block_number(hash)
            .await?
            .expect("Finalized block has a number.");

        if best_finalized_block_number >= not_older_than {
            return Ok(best_finalized_block_number);
        }

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
                    match is_halted {
                        Ok(is_halted) => {
                            if is_halted {
                                circuit_breaker_sender.send(CircuitBreakerEvent::BridgeHaltAlephZero)?;
                                warn!(target: "AlephZeroHaltedListener",
                                      "Most is halted, exiting");
                                return Ok(CircuitBreakerEvent::BridgeHaltAlephZero);
                            }
                        },

                        Err(why) => {
                            warn!("Exiting due to a connection error {why:?}");
                            let status = CircuitBreakerEvent::AlephClientError;
                            circuit_breaker_sender.send(status.clone())?;
                            return Ok(status.clone());
                        }
                    }

                    // sleep before making another query
                    sleep(Duration::from_secs(ALEPH_BLOCK_PROD_TIME_SEC)).await;
                }
            }
        }
    }
}
