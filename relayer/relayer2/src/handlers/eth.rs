use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
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
        mpsc::{self, error::SendError},
        Mutex,
    },
    task::JoinHandle,
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::EthConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
    listeners::EthMostEvents,
    CircuitBreakerEvent,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthHandlerError {
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("channel send error")]
    Send(#[from] SendError<u32>),
}

pub struct EthHandler;

impl EthHandler {
    // pub async fn run(
    //     mut eth_events_receiver: mpsc::Receiver<EthMostEvents>,
    //     config: Arc<Config>,
    //     azero_connection: Arc<AzeroConnectionWithSigner>,
    //     // is_circuit_open: Arc<AtomicBool>,
    // ) -> Result<(), EthHandlerError> {
    //     loop {
    //         if let Some(eth_events) = eth_events_receiver.recv().await {
    //             // handle each block in a separate task
    //             tokio::spawn(handle_events(
    //                 eth_events,
    //                 Arc::clone(&config),
    //                 Arc::clone(&azero_connection),
    //             ));
    //         }
    //     }
    // }

    // pub async fn handle_events(
    //     events: EthMostEvents,
    //     config: Arc<Config>,
    //     azero_connection: Arc<AzeroConnectionWithSigner>,
    // ) -> Result<(), EthHandlerError> {
    //     let EthMostEvents { events, ack_sender } = events;

    //     for event in events {
    //         handle_event(event, &config, &azero_connection).await?;
    //     }
    //     // we processed all the events in this block
    //     info!("All events processed, sending acknowledgement");
    //     _ = ack_sender.send(());

    //     Ok(())
    // }

    pub async fn handle_event(
        event: MostEvents,
        config: &Config,
        azero_connection: &AzeroConnectionWithSigner,
    ) -> Result<(), EthHandlerError> {
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
