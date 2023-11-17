use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

use aleph_client::{
    contract::event::{translate_events, BlockDetails, ContractEvent},
    contract_transcode::Value,
    utility::BlocksApi,
    AlephConfig, AsConnection,
};
use ethers::{
    abi::{self, EncodePackedError, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    types::U256,
    utils::keccak256,
};
use log::{debug, info, trace, warn};
use redis::{aio::Connection as RedisConnection, AsyncCommands, RedisError};
use subxt::{events::Events, utils::H256};
use thiserror::Error;
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::{
        azero::SignedAzeroWsConnection,
        eth::{EthConnectionError, EthWsConnection, SignedEthWsConnection},
    },
    contracts::{AzeroContractError, Membrane, MembraneInstance},
    listeners::eth::get_next_finalized_block_number_eth,
};

const ALEPH_LAST_BLOCK_KEY: &str = "alephzero_last_known_block_number";

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("subxt error")]
    Subxt(#[from] subxt::Error),

    #[error("azero provider error")]
    Provider(#[from] ProviderError),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("eth connection error")]
    EthConnection(#[from] EthConnectionError),

    #[error("eth contract error")]
    EthContractListen(#[from] ContractError<EthWsConnection>),

    #[error("eth contract error")]
    EthContractTx(#[from] ContractError<SignedEthWsConnection>),

    #[error("no block found")]
    BlockNotFound,

    #[error("no tx receipt")]
    NoTxReceipt,

    #[error("eth tx not found")]
    EthTxNotFound,

    #[error("missing data from event")]
    MissingEventData(String),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("redis connection error")]
    Redis(#[from] RedisError),

    #[error("unexpected error")]
    Unexpected,
}

const BLOCK_PROD_TIME_SEC: u64 = 1;
const ETH_FINALITY_WAIT_SEC: u64 = 300;

pub struct AlephZeroListener;

impl AlephZeroListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
        redis_connection: Arc<Mutex<RedisConnection>>,
    ) -> Result<(), AzeroListenerError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            name,
            default_sync_from_block_azero,
            sync_step,
            ..
        } = &*config;

        let pending_blocks: Arc<Mutex<BTreeSet<u32>>> = Arc::new(Mutex::new(BTreeSet::new()));

        let instance = Arc::new(MembraneInstance::new(
            azero_contract_address,
            azero_contract_metadata,
        )?);
        let mut first_unprocessed_block_number = read_first_unprocessed_block_number(
            name.clone(),
            redis_connection.clone(),
            *default_sync_from_block_azero,
        )
        .await;

        // Add the first block number to the set of pending blocks.
        add_to_pending(first_unprocessed_block_number, pending_blocks.clone()).await;

        // Main AlephZero event loop
        loop {
            // Query for the next unknowns finalized block number, if not present we wait for it.
            let next_finalized_block_number = get_next_finalized_block_number_azero(
                azero_connection.clone(),
                first_unprocessed_block_number,
            )
            .await;

            // Check at most `sync_step` blocks before caching.
            let to_block = std::cmp::min(
                next_finalized_block_number,
                first_unprocessed_block_number + sync_step - 1,
            );

            log::info!(
                "Processing events from blocks {} - {}",
                first_unprocessed_block_number,
                to_block
            );

            // Process events from the next unknowns finalized block number.
            for block_number in first_unprocessed_block_number..=to_block {
                // Add the next block number now, so that there is always some block number in the set.
                add_to_pending(block_number + 1, pending_blocks.clone()).await;

                let block_hash = azero_connection
                    .get_block_hash(block_number)
                    .await?
                    .ok_or(AzeroListenerError::BlockNotFound)?;

                let events = azero_connection
                    .as_connection()
                    .as_client()
                    .blocks()
                    .at(block_hash)
                    .await?
                    .events()
                    .await?;

                let config = config.clone();
                let eth_connection = eth_connection.clone();
                let redis_connection = redis_connection.clone();
                let pending_blocks = pending_blocks.clone();
                let membrane_instance = instance.clone();

                // filter contract events
                tokio::spawn(async move {
                    handle_events(
                        eth_connection,
                        &config,
                        events,
                        membrane_instance,
                        block_number,
                        block_hash,
                        pending_blocks.clone(),
                        redis_connection.clone(),
                    )
                    .await
                    .expect("Block events handler failed");
                });
            }

            // Update the last block number.
            first_unprocessed_block_number = to_block + 1;
        }
    }
}

async fn add_to_pending(block_number: u32, pending_blocks: Arc<Mutex<BTreeSet<u32>>>) {
    let mut pending_blocks = pending_blocks.lock().await;
    pending_blocks.insert(block_number);
}

// handle all events present in one block
async fn handle_events(
    eth_connection: Arc<SignedEthWsConnection>,
    config: &Config,
    events: Events<AlephConfig>,
    membrane_instance: Arc<MembraneInstance>,
    block_number: u32,
    block_hash: H256,
    pending_blocks: Arc<Mutex<BTreeSet<u32>>>,
    redis_connection: Arc<Mutex<RedisConnection>>,
) -> Result<(), AzeroListenerError> {
    let Config {
        name,
        eth_contract_address,
        ..
    } = config;
    let contracts = &[&membrane_instance.contract];
    let mut event_tasks = Vec::new();
    for event_res in translate_events(
        events.iter(),
        contracts,
        Some(BlockDetails {
            block_number,
            block_hash,
        }),
    ) {
        let event = event_res?;
        let eth_contract_address = eth_contract_address.clone();
        let eth_connection = eth_connection.clone();
        // Spawn a new task for handling each event.
        event_tasks.push(tokio::spawn(async move {
            handle_event(eth_connection, eth_contract_address, event)
                .await
                .expect("Event handler failed");
        }));
    }

    // Wait for all event processing tasks to finish.
    for task in event_tasks {
        task.await.expect("Event processing task has failed");
    }

    // Lock the pending blocks set and remove the current block number (as we managed to process all events from it).
    let mut pending_blocks = pending_blocks.lock().await;
    pending_blocks.remove(&block_number);

    // Now we know that all blocks before the pending block with the lowest number have been processed.
    // We can update the last processed block number in Redis.
    let earliest_still_pending = pending_blocks
        .first()
        .expect("There should always be a pending block in the set)");

    // Note: `earliest_still_pending` will never be 0
    write_last_processed_block(name.clone(), redis_connection, earliest_still_pending - 1).await?;

    Ok(())
}

fn get_event_data(
    data: &HashMap<String, Value>,
    field: &str,
) -> Result<[u8; 32], AzeroListenerError> {
    match data.get(field) {
        Some(Value::Hex(hex)) => {
            let mut result = [0u8; 32];
            result.copy_from_slice(hex.bytes());
            Ok(result)
        }
        _ => Err(AzeroListenerError::Unexpected),
    }
}

async fn handle_event(
    eth_connection: Arc<SignedEthWsConnection>,
    eth_contract_address: String,
    event: ContractEvent,
) -> Result<(), AzeroListenerError> {
    if let Some(name) = &event.name {
        if name.eq("CrosschainTransferRequest") {
            info!("handling A0 contract event: {event:?}");

            let data = event.data;

            // decode event data
            let dest_token_address = get_event_data(&data, "dest_token_address")?;
            let amount = get_event_data(&data, "amount")?;
            let dest_receiver_address = get_event_data(&data, "dest_receiver_address")?;
            let request_nonce = get_event_data(&data, "request_nonce")?;

            let amount = U256::from_little_endian(&amount);
            let request_nonce = U256::from_little_endian(&request_nonce);

            // hash event data
            let bytes = abi::encode_packed(&[
                Token::FixedBytes(dest_token_address.to_vec()),
                Token::Int(amount),
                Token::FixedBytes(dest_receiver_address.to_vec()),
                Token::Int(request_nonce),
            ])?;

            trace!("ABI event encoding: {bytes:?}");

            let request_hash = keccak256(bytes);

            debug!("hashed event encoding: {request_hash:?}");

            let address = eth_contract_address.parse::<Address>()?;
            let contract = Membrane::new(address, eth_connection.clone());

            //  forward transfer & vote
            let call: ContractCall<SignedEthWsConnection, ()> = contract.receive_request(
                request_hash,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
            );

            let tx_hash = call
                .send()
                .await?
                .await?
                .ok_or(AzeroListenerError::NoTxReceipt)?
                .transaction_hash;

            wait_for_eth_tx_finality(eth_connection, tx_hash).await?;
        }
    }
    Ok(())
}

pub async fn wait_for_eth_tx_finality(
    eth_connection: Arc<SignedEthWsConnection>,
    tx_hash: H256,
) -> Result<(), AzeroListenerError> {
    loop {
        log::info!("Waiting for tx finality: {tx_hash:?}");
        sleep(Duration::from_secs(ETH_FINALITY_WAIT_SEC)).await;

        let finalized_head_number =
            get_next_finalized_block_number_eth(eth_connection.clone(), 0).await;
        let tx_opt = eth_connection
            .get_transaction(tx_hash)
            .await
            .map_err(|_err| AzeroListenerError::EthTxNotFound)?;
        if let Some(tx) = tx_opt {
            // If the tx is not in a block yet, keep waiting.
            if tx.block_number.is_none() {
                continue;
            }

            if tx.block_number.expect("Tx is included in some block.")
                <= finalized_head_number.into()
            {
                log::info!("Eth tx finalized: {tx_hash:?}");
                return Ok(());
            }
        }
    }
}

pub async fn get_next_finalized_block_number_azero(
    azero_connection: Arc<SignedAzeroWsConnection>,
    not_older_than: u32,
) -> u32 {
    let mut best_finalized_block_number_opt: Option<u32>;
    loop {
        best_finalized_block_number_opt = match azero_connection.get_finalized_block_hash().await {
            Ok(hash) => match azero_connection.get_block_number(hash).await {
                Ok(number_opt) => number_opt,
                Err(err) => {
                    warn!("Aleph Client error when getting best finalized block number: {err}");
                    None
                }
            },
            Err(err) => {
                warn!("Aleph Client error when getting best finalized block hash: {err}");
                None
            }
        };

        if let Some(best_finalized_block_number) = best_finalized_block_number_opt {
            if best_finalized_block_number >= not_older_than {
                break;
            }
        }

        sleep(Duration::from_secs(BLOCK_PROD_TIME_SEC)).await;
    }
    best_finalized_block_number_opt.expect("We return only if we managed to fetch the block.")
}

async fn read_first_unprocessed_block_number(
    name: String,
    redis_connection: Arc<Mutex<RedisConnection>>,
    default_block: u32,
) -> u32 {
    let mut connection = redis_connection.lock().await;

    match connection
        .get::<_, u32>(format!("{name}:{ALEPH_LAST_BLOCK_KEY}"))
        .await
    {
        Ok(value) => value + 1,
        Err(why) => {
            warn!("Redis connection error {why:?}");
            default_block
        }
    }
}

async fn write_last_processed_block(
    name: String,
    redis_connection: Arc<Mutex<RedisConnection>>,
    last_block_number: u32,
) -> Result<(), AzeroListenerError> {
    let mut connection = redis_connection.lock().await;
    connection
        .set(format!("{name}:{ALEPH_LAST_BLOCK_KEY}"), last_block_number)
        .await?;
    Ok(())
}
