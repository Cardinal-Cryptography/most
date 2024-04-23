use std::{str::FromStr, sync::Arc};

use aleph_client::{waiting::BlockStatus, AsConnection, SignedConnectionApi};
use ethers::{core::types::H256, utils::keccak256};
use log::{debug, error, info, trace, warn};
use rustc_hex::FromHexError;
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc},
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::azero::AzeroConnectionWithSigner,
    contracts::{AzeroContractError, CrosschainTransferRequestFilter, MostEvents, MostInstance},
    helpers::concat_u8_arrays,
    listeners::EthMostEvents,
    AccountId, CircuitBreakerEvent,
};

// Frequency of checking for finality of the transaction
const AZERO_WAIT_FOR_FINALITY_CHECK: Duration = Duration::from_millis(1000);

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumEventHandlerError {
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("receive_request tx has failed")]
    ReceiveRequestTxFailure {
        request_hash: String,
        committee_id: u128,
        dest_token_address: String,
        amount: u128,
        dest_receiver_address: String,
        request_nonce: u128,
    },

    #[error("error when decoding a hex encoded string")]
    FromHex(#[from] FromHexError),

    #[error("Bridge misconfiguration: committee id mismatch")]
    CommitteeIdMismatch,
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
            blacklisted_requests,
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
            debug!("Handling eth contract event: {crosschain_transfer_event:?}");

            info!(
                    "Decoded event data: [dest_token_address: 0x{}, amount: {amount}, dest_receiver_address: 0x{}, request_nonce: {request_nonce}, committee_id: {committee_id}]",
                    AccountId::from(dest_token_address),
                    AccountId::from(dest_receiver_address)
                );

            // concat bytes
            let bytes = concat_u8_arrays(vec![
                &committee_id.as_u128().to_le_bytes(),
                &dest_token_address,
                &amount.as_u128().to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.as_u128().to_le_bytes(),
            ]);

            trace!("Concatenated event bytes: {bytes:?}");

            let request_hash = keccak256(bytes);
            debug!("Hashed event data: {request_hash:?}");

            let request_hash_hex = hex::encode(request_hash);
            info!("Request hash hex encoding: 0x{}", request_hash_hex);

            if let Some(blacklist) = blacklisted_requests {
                if blacklist.contains(&H256::from_str(&request_hash_hex)?) {
                    warn!("Skipping blacklisted request: 0x{request_hash_hex}");
                    return Ok(());
                }
            }

            let contract = MostInstance::new(
                azero_contract_address,
                azero_contract_metadata,
                config.azero_ref_time_limit,
                config.azero_proof_size_limit,
            )?;

            let committee_id = committee_id.as_u128();
            let amount = amount.as_u128();
            let request_nonce = request_nonce.as_u128();

            if not_in_committee(&contract, azero_connection, committee_id).await? {
                info!("Guardian signature for 0x{request_hash_hex} not needed - request from a different committee");
                return Ok(());
            }

            while contract
                .needs_signature(
                    azero_connection.as_connection(),
                    request_hash,
                    azero_connection.account_id().clone(),
                    committee_id,
                    BlockStatus::Finalized,
                )
                .await?
            {
                info!("Azero: request with nonce {request_nonce} not yet finalized.");

                if !contract
                    .needs_signature(
                        azero_connection.as_connection(),
                        request_hash,
                        azero_connection.account_id().clone(),
                        committee_id,
                        BlockStatus::Best,
                    )
                    .await?
                {
                    sleep(AZERO_WAIT_FOR_FINALITY_CHECK).await;
                    continue;
                }
                // send vote
                contract
                    .receive_request(
                        azero_connection,
                        request_hash,
                        committee_id,
                        dest_token_address,
                        amount,
                        dest_receiver_address,
                        request_nonce,
                    )
                    .await
                    // default AlephClient error is MBs large and useless, dumps the entire runtime for some reason
                    .map_err(|_| EthereumEventHandlerError::ReceiveRequestTxFailure {
                        request_hash: hex::encode(request_hash),
                        committee_id,
                        dest_token_address: hex::encode(dest_token_address),
                        amount,
                        dest_receiver_address: hex::encode(dest_receiver_address),
                        request_nonce,
                    })?;
            }
            info!("Guardian signature for 0x{request_hash_hex} no longer needed");
        }

        Ok(())
    }
}

async fn not_in_committee(
    most: &MostInstance,
    connection: &AzeroConnectionWithSigner,
    committee_id: u128,
) -> Result<bool, EthereumEventHandlerError> {
    if most
        .is_in_committee(
            connection.as_connection(),
            committee_id,
            connection.account_id().clone(),
        )
        .await?
    {
        return Ok(false);
    }

    if committee_id
        > most
            .current_committee_id(connection.as_connection())
            .await?
    {
        error!("Request from a future committee {committee_id} - this likely indicates MOST contracts misconfiguration");
        return Err(EthereumEventHandlerError::CommitteeIdMismatch);
    }
    Ok(true)
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
        mut eth_events_receiver: mpsc::Receiver<EthMostEvents>,
        azero_signed_connection: Arc<AzeroConnectionWithSigner>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, EthereumEventsHandlerError> {
        info!("Starting");

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
                        from_block,
                        to_block
                    } = eth_events;

                    info!("Received a batch of {} events from blocks {from_block} to {to_block}", events.len());

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
