use std::sync::Arc;

use ethers::utils::keccak256;
use log::{debug, error, info, trace, warn};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, Mutex},
};

use crate::{
    config::Config,
    connections::azero::AzeroConnectionWithSigner,
    contracts::{AzeroContractError, CrosschainTransferRequestFilter, MostEvents, MostInstance},
    helpers::concat_u8_arrays,
    listeners::EthMostEvents,
    CircuitBreakerEvent,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumEventHandlerError {
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),
}

pub struct EthereumEventHandler;

impl EthereumEventHandler {
    pub async fn handle_event(
        event: MostEvents,
        config: &Config,
        azero_connection: &AzeroConnectionWithSigner,
    ) -> Result<(), EthereumEventHandlerError> {
        let Config {
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        if let MostEvents::CrosschainTransferRequestFilter(
            crosschain_transfer_event @ CrosschainTransferRequestFilter {
                committee_id,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
                ..
            },
        ) = event
        {
            info!("handling eth contract event: {crosschain_transfer_event:?}");

            // concat bytes
            let bytes = concat_u8_arrays(vec![
                &committee_id.as_u128().to_le_bytes(),
                &dest_token_address,
                &amount.as_u128().to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.as_u128().to_le_bytes(),
            ]);

            trace!("event concatenated bytes: {bytes:?}");

            let request_hash = keccak256(bytes);
            debug!("hashed event encoding: {request_hash:?}");

            let contract = MostInstance::new(
                azero_contract_address,
                azero_contract_metadata,
                config.azero_ref_time_limit,
                config.azero_proof_size_limit,
            )?;

            // send vote
            contract
                .receive_request(
                    azero_connection,
                    request_hash,
                    committee_id.as_u128(),
                    dest_token_address,
                    amount.as_u128(),
                    dest_receiver_address,
                    request_nonce.as_u128(),
                )
                .await?;
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumEventsHandlerError {
    #[error("events ack receiver dropped")]
    EventsAckReceiverDropped,

    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),
}

pub struct EthereumEventsHandler;

impl EthereumEventsHandler {
    pub async fn run(
        config: Arc<Config>,
        eth_events_receiver: Arc<Mutex<mpsc::Receiver<EthMostEvents>>>,
        azero_signed_connection: Arc<AzeroConnectionWithSigner>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, EthereumEventsHandlerError> {
        let mut eth_events_receiver = eth_events_receiver.lock().await;

        loop {
            debug!("Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                Some(eth_events) = eth_events_receiver.recv() => {
                    let EthMostEvents {
                        events,
                        events_ack_sender,
                    } = eth_events;
                    info!("Received a batch of {} events", events.len());

                    for event in events {
                        select! {
                            cb_event = circuit_breaker_receiver.recv () => {
                                warn!("Exiting due to a circuit breaker event {cb_event:?}");
                                return Ok(cb_event?);
                            },

                            result = EthereumEventHandler::handle_event(event, &config, &azero_signed_connection) => {
                                if let Err(why) = result {
                                    circuit_breaker_sender.send(CircuitBreakerEvent::EthEventHandlerFailure)?;
                                    warn!("Event handler failed {why:?}, exiting");
                                    return Ok (CircuitBreakerEvent::EthEventHandlerFailure);
                                }
                            },

                        }
                    }

                    info!("Acknowledging events batch");
                    // marks the batch as done and releases the listener
                    events_ack_sender
                        .send(())
                        .map_err(|_| EthereumEventsHandlerError::EventsAckReceiverDropped)?;

                }

            }
        }
    }
}