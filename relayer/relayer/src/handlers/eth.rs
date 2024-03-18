use ethers::utils::keccak256;
use log::{debug, error, info, trace};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot};

use crate::{
    config::Config,
    connections::azero::AzeroConnectionWithSigner,
    contracts::{AzeroContractError, CrosschainTransferRequestFilter, MostEvents, MostInstance},
    helpers::concat_u8_arrays,
    listeners::{EthMostEvent, EthMostEvents},
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
    #[error("event channel send error")]
    EventSend(#[from] mpsc::error::SendError<EthMostEvent>),

    #[error("events channel send error")]
    EventsSend(#[from] mpsc::error::SendError<EthMostEvents>),

    #[error("ack receive error")]
    AckReceive(#[from] oneshot::error::RecvError),

    #[error("events ack receiver dropped")]
    EventsAckReceiverDropped,
}

pub struct EthereumEventsHandler;

impl EthereumEventsHandler {
    pub async fn run(
        mut eth_events_receiver: mpsc::Receiver<EthMostEvents>,
        eth_event_sender: mpsc::Sender<EthMostEvent>,
    ) -> Result<(), EthereumEventsHandlerError> {
        loop {
            if let Some(eth_events) = eth_events_receiver.recv().await {
                let EthMostEvents {
                    events,
                    events_ack_sender,
                } = eth_events;
                info!("[Ethereum] Received a batch of {} events", events.len());

                for event in events {
                    let (event_ack_sender, event_ack_receiver) = oneshot::channel::<()>();
                    info!("[Ethereum] Sending event {event:?}");
                    eth_event_sender
                        .send(EthMostEvent {
                            event,
                            event_ack_sender,
                        })
                        .await?;
                    info!("[Ethereum] Awaiting event ack");
                    // wait until this event is handled before proceeding.
                    event_ack_receiver.await?;
                    info!("[Ethereum] Event ack received");
                }

                info!("[Ethereum] Acknowledging events batch");
                // marks the batch as done and releases the listener
                events_ack_sender
                    .send(())
                    .map_err(|_| EthereumEventsHandlerError::EventsAckReceiverDropped)?;
            }
        }
    }
}
