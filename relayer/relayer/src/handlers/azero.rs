use std::sync::Arc;

use aleph_client::contract::event::ContractEvent;
use ethers::{
    abi::{self, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    types::{BlockNumber, U64},
    utils::keccak256,
};
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc},
    task::{JoinError, JoinSet},
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::eth::SignedEthConnection,
    contracts::{get_request_event_data, AzeroContractError, CrosschainTransferRequestData, Most},
    listeners::AzeroMostEvents,
    CircuitBreakerEvent,
};

// Frequency of checking for finality of the transaction
const ETH_WAIT_FOR_FINALITY_CHECK_SEC: u64 = 60;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroEventHandlerError {
    #[error("Error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("Ethers provider error")]
    Provider(#[from] ProviderError),

    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Eth contract error")]
    EthContractTx(#[from] ContractError<SignedEthConnection>),

    #[error("Tx was not present in any block or mempool after the maximum number of retries")]
    TxNotPresentInBlockOrMempool,

    #[error("Contract reverted")]
    EthContractReverted,
}

pub struct AlephZeroEventHandler;

impl AlephZeroEventHandler {
    pub async fn handle_event(
        event: ContractEvent,
        config: Arc<Config>,
        eth_signed_connection: Arc<SignedEthConnection>,
    ) -> Result<(), AlephZeroEventHandlerError> {
        let Config {
            eth_contract_address,
            eth_tx_min_confirmations,
            eth_tx_submission_retries,
            ..
        } = &*config;

        if let Some(name) = &event.name {
            if name.eq("CrosschainTransferRequest") {
                let data = event.data;

                // decode event data
                let CrosschainTransferRequestData {
                    committee_id,
                    dest_token_address,
                    amount,
                    dest_receiver_address,
                    request_nonce,
                } = get_request_event_data(&data)?;

                info!(
                    "Decoded event data: [dest_token_address: 0x{}, amount: {amount}, dest_receiver_address: 0x{}, request_nonce: {request_nonce}]",
                    hex::encode(dest_token_address),
                    hex::encode(dest_receiver_address)
                );

                // NOTE: for some reason, ethers-rs's `encode_packed` does not properly encode the data
                // (it does not pad uint to 32 bytes, but uses the actual number of bytes required to store the value)
                // so we use `abi::encode` instead (it only differs for signed and dynamic size types, which we don't use here)
                let bytes = abi::encode(&[
                    Token::Uint(committee_id.into()),
                    Token::FixedBytes(dest_token_address.to_vec()),
                    Token::Uint(amount.into()),
                    Token::FixedBytes(dest_receiver_address.to_vec()),
                    Token::Uint(request_nonce.into()),
                ]);

                debug!("ABI event encoding: 0x{}", hex::encode(bytes.clone()));

                let request_hash = keccak256(bytes);
                let request_hash_hex = hex::encode(request_hash);

                info!("hashed event encoding: 0x{}", request_hash_hex);

                let address = eth_contract_address.parse::<Address>()?;
                let contract = Most::new(address, eth_signed_connection.clone());

                while contract
                    .needs_signature(
                        request_hash,
                        eth_signed_connection.address(),
                        committee_id.into(),
                    )
                    .block(BlockNumber::Finalized)
                    .await?
                {
                    info!("Request with nonce {} not yet finalized.", request_nonce);

                    if !contract
                        .needs_signature(
                            request_hash,
                            eth_signed_connection.address(),
                            committee_id.into(),
                        )
                        .block(BlockNumber::Latest)
                        .await?
                    {
                        sleep(Duration::from_secs(ETH_WAIT_FOR_FINALITY_CHECK_SEC)).await;
                        continue;
                    }

                    // forward transfer & vote
                    let call: ContractCall<SignedEthConnection, ()> = contract.receive_request(
                        request_hash,
                        committee_id.into(),
                        dest_token_address,
                        amount.into(),
                        dest_receiver_address,
                        request_nonce.into(),
                    );

                    debug!("Dry-running tx with nonce {}", request_nonce);

                    // Dry-run the tx to check for potential reverts.
                    call.clone().gas(config.eth_gas_limit).call().await?;

                    info!(
                        "Sending tx with nonce {} to the Ethereum network and waiting for {} confirmations.",
                        request_nonce,
                        eth_tx_min_confirmations
                    );

                    let receipt = call
                        .gas(config.eth_gas_limit)
                        .nonce(eth_signed_connection.inner().next())
                        .send()
                        .await?
                        .confirmations(*eth_tx_min_confirmations)
                        .retries(*eth_tx_submission_retries)
                        .await?
                        .ok_or(AlephZeroEventHandlerError::TxNotPresentInBlockOrMempool)?;

                    let tx_hash = receipt.transaction_hash;
                    let tx_status = receipt.status;

                    // Check if the tx reverted.
                    if tx_status == Some(U64::from(0)) {
                        warn!(
                        "Tx with nonce {request_nonce} has been sent to the Ethereum network: {tx_hash:?} but it reverted."
                    );
                        return Err(AlephZeroEventHandlerError::EthContractReverted);
                    }

                    info!("Tx with nonce {request_nonce} has been sent to the Ethereum network: {tx_hash:?} and received {eth_tx_min_confirmations} confirmations.");
                }

                info!("Guardian signature for {request_hash:?} no longer needed");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroEventsHandlerError {
    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<u32>),

    #[error("task join error")]
    Join(#[from] JoinError),

    #[error("ack receiver dropped before response could be sent")]
    AckSend,
}

pub struct AlephZeroEventsHandler;

impl AlephZeroEventsHandler {
    pub async fn run(
        config: Arc<Config>,
        eth_signed_connection: Arc<SignedEthConnection>,
        mut azero_events_receiver: mpsc::Receiver<AzeroMostEvents>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AlephZeroEventsHandlerError> {
        loop {
            debug!("Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                Some(azero_events) = azero_events_receiver.recv() => {
                    let AzeroMostEvents {
                        events,
                        from_block,
                        to_block,
                        ack,
                    } = azero_events;

                    info!("Received a batch of {} events from blocks {from_block} to {to_block}", events.len());

                    let config = Arc::clone(&config);
                    let eth_signed_connection = Arc::clone(&eth_signed_connection);
                    let circuit_breaker_sender = circuit_breaker_sender.clone ();

                    // spawn non-blocking task to handle all events w-out blocking the events publisher
                    tokio::spawn(async move {
                        let mut tasks = JoinSet::new();
                        for event in events {
                            // spawn each handler in separate task as it's time consuming
                            tasks.spawn(AlephZeroEventHandler::handle_event(
                                event,
                                Arc::clone(&config),
                                Arc::clone(&eth_signed_connection),
                            ));
                        }

                        // wait for all concurrent handler tasks to finish
                        info!("Awaiting all event handler tasks for blocks {}-{} to finish", from_block, to_block);

                        while let Some(result) = tasks.join_next().await {
                            match result? {
                                Ok(_) => {},
                                Err(why) => {
                                    warn!("Event handler failed {why:?}, opening circuit breaker");
                                    circuit_breaker_sender.send(CircuitBreakerEvent::AlephZeroEventHandlerFailure)?;
                                },
                            }
                        }

                        ack.send(to_block).map_err(|_| AlephZeroEventsHandlerError::AckSend)?;
                        Ok::<(), AlephZeroEventsHandlerError> (())
                    });

                }
            }
        }
    }
}
