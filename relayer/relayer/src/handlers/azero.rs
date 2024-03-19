use std::sync::Arc;

use aleph_client::contract::event::ContractEvent;
use ethers::{
    abi::{self, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    types::U64,
    utils::keccak256,
};
use log::{debug, error, info, warn};
use subxt::utils::H256;
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, Mutex},
    task::{JoinError, JoinSet},
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::eth::SignedEthConnection,
    contracts::{get_request_event_data, AzeroContractError, CrosschainTransferRequestData, Most},
    listeners::{get_next_finalized_block_number_eth, AzeroMostEvents, ETH_BLOCK_PROD_TIME_SEC},
    CircuitBreakerEvent,
};

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
        eth_connection: Arc<SignedEthConnection>,
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

                info!("hashed event encoding: 0x{}", hex::encode(request_hash));

                let address = eth_contract_address.parse::<Address>()?;
                let contract = Most::new(address, eth_connection.clone());

                // forward transfer & vote
                let call: ContractCall<SignedEthConnection, ()> = contract.receive_request(
                    request_hash,
                    committee_id.into(),
                    dest_token_address,
                    amount.into(),
                    dest_receiver_address,
                    request_nonce.into(),
                );

                info!(
                "Sending tx with request nonce {} to the Ethereum network and waiting for {} confirmations.",
                request_nonce,
                eth_tx_min_confirmations
            );

                // This shouldn't fail unless there is something wrong with our config.
                // NOTE: this does not check whether the actual tx reverted on-chain. Reverts are only checked on dry-run.
                let receipt = call
                    .gas(config.eth_gas_limit)
                    .nonce(eth_connection.inner().next())
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

                wait_for_eth_tx_finality(eth_connection, tx_hash).await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroEventsHandlerError {
    #[error("events ack receiver dropped")]
    EventsAckReceiverDropped,

    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("Task join error")]
    Join(#[from] JoinError),
}

pub struct AlephZeroEventsHandler;

impl AlephZeroEventsHandler {
    pub async fn run(
        config: Arc<Config>,
        eth_signed_connection: Arc<SignedEthConnection>,
        azero_events_receiver: Arc<Mutex<mpsc::Receiver<AzeroMostEvents>>>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AlephZeroEventsHandlerError> {
        let mut azero_events_receiver = azero_events_receiver.lock().await;

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
                        events_ack_sender,
                    } = azero_events;

                    info!("Received a batch of {} events", events.len());

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
                    info!("Awaiting all handler tasks to finish");

                    while !tasks.is_empty() {
                        select! {
                            cb_event = circuit_breaker_receiver.recv() => {
                                warn!("Exiting due to a circuit breaker event {cb_event:?}");
                                return Ok(cb_event?);
                            },

                            Some (result) = tasks.join_next() => {
                                if let Err (why) = result {
                                    circuit_breaker_sender.send(CircuitBreakerEvent::AlephZeroEventHandlerFailure)?;
                                    warn!("Event handler failed {why:?}, exiting");
                                    return Ok(CircuitBreakerEvent::AlephZeroEventHandlerFailure);
                                }
                            }
                        }
                    }

                    info!("Acknowledging events batch");
                    // marks the batch as done and releases the listener
                    events_ack_sender
                        .send(())
                        .map_err(|_| AlephZeroEventsHandlerError::EventsAckReceiverDropped)?;
                    info!("All events acknowledged");
                }
            }
        }
    }
}

async fn wait_for_eth_tx_finality(
    eth_connection: Arc<SignedEthConnection>,
    tx_hash: H256,
) -> Result<(), AlephZeroEventHandlerError> {
    info!("Waiting for tx finality: {tx_hash:?}");
    loop {
        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;

        let connection_rc = Arc::new(eth_connection.provider().clone());
        let finalized_head_number = get_next_finalized_block_number_eth(connection_rc, 0).await;

        match eth_connection.get_transaction(tx_hash).await {
            Ok(Some(tx)) => {
                if let Some(block_number) = tx.block_number {
                    if block_number <= finalized_head_number.into() {
                        info!("Eth tx {tx_hash:?} finalized");
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                error!("Failed to get tx that should be present: {err}");
            }
            Ok(None) => panic!("Transaction {tx_hash:?} for which finality we were waiting is no longer included in the chain, aborting..."),
        };
    }
}