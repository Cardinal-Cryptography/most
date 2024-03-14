use std::{
    cmp::min,
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    utility::BlocksApi,
    AlephConfig, AsConnection, Connection,
};
use ethers::{
    abi::{self, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    types::U64,
    utils::keccak256,
};
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, RedisError};
use subxt::{events::Events, utils::H256};
use thiserror::Error;
use tokio::{
    sync::{broadcast, mpsc, oneshot, AcquireError, Mutex, OwnedSemaphorePermit, Semaphore},
    task::{JoinError, JoinSet},
    time::{sleep, Duration},
};

use super::AzeroMostEvents;
use crate::{
    config::Config,
    connections::{azero::AzeroWsConnection, eth::SignedEthConnection},
    contracts::{
        get_request_event_data, AzeroContractError, CrosschainTransferRequestData, Most,
        MostInstance,
    },
    listeners::eth::{get_next_finalized_block_number_eth, ETH_BLOCK_PROD_TIME_SEC},
};

const ALEPH_BLOCK_PROD_TIME_SEC: u64 = 1;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {
    #[error("Aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("Error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("Subxt error")]
    Subxt(#[from] subxt::Error),

    #[error("Ethers provider error")]
    Provider(#[from] ProviderError),

    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Eth contract error")]
    EthContractTx(#[from] ContractError<SignedEthConnection>),

    #[error("No block found")]
    BlockNotFound,

    #[error("Tx was not present in any block or mempool after the maximum number of retries")]
    TxNotPresentInBlockOrMempool,

    #[error("Missing data from event")]
    MissingEventData(String),

    // #[error("Bridge was halted, restart required")]
    // BridgeHaltedRestartRequired,

    // #[error("Redis connection error")]
    // Redis(#[from] RedisError),
    #[error("Join error")]
    Join(#[from] JoinError),

    #[error("Semaphore error")]
    Semaphore(#[from] AcquireError),

    #[error("Contract reverted")]
    EthContractReverted,

    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<AzeroMostEvents>),

    #[error("channel receive error")]
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
    ) -> Result<(), AzeroListenerError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            azero_ref_time_limit,
            azero_proof_size_limit,
            azero_max_event_handler_tasks,
            eth_contract_address,
            default_sync_from_block_azero,
            name,
            sync_step,
            ..
        } = &*config;

        let pending_blocks: Arc<Mutex<BTreeSet<u32>>> = Arc::new(Mutex::new(BTreeSet::new()));
        let event_handler_tasks_semaphore =
            Arc::new(Semaphore::new(*azero_max_event_handler_tasks));
        // let mut block_sealing_tasks = JoinSet::new();

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        // let most_eth_address = eth_contract_address.parse::<Address>()?;
        // let most_eth = Most::new(most_eth_address, eth_connection.clone());

        // let mut first_unprocessed_block_number = read_first_unprocessed_block_number(
        //     name.clone(),
        //     ALEPH_LAST_BLOCK_KEY.to_string(),
        //     redis_connection.clone(),
        //     **default_sync_from_block_azero,
        // )
        // .await;

        // Add the first block number to the set of pending blocks.
        // add_to_pending(first_unprocessed_block_number, pending_blocks.clone()).await;

        loop {
            let unprocessed_block_number = next_unprocessed_block_number.recv().await?;

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

            info!(
                "Processing events from blocks {} - {}",
                unprocessed_block_number, to_block
            );

            // Add the next block numbers now, so that there is always some block number in the set.
            for block_number in unprocessed_block_number..=to_block {
                add_to_pending(block_number + 1, pending_blocks.clone()).await;
            }

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

            let (events_ack_sender, events_ack_receiver) = oneshot::channel::<()>();

            info!("Sending a batch of {} events", filtered_events.len());

            azero_events_sender
                .send(AzeroMostEvents {
                    events: filtered_events,
                    events_ack_sender,
                })
                .await?;

            info!("Awaiting ack");
            _ = events_ack_receiver.await;

            // publish this block number as processed
            last_processed_block_number.send(unprocessed_block_number)?;
        }

        Ok(())
    }
}

async fn add_to_pending(block_number: u32, pending_blocks: Arc<Mutex<BTreeSet<u32>>>) {
    let mut pending_blocks = pending_blocks.lock().await;
    pending_blocks.insert(block_number);
}

async fn fetch_events_in_block_range(
    azero_connection: Arc<AzeroWsConnection>,
    from_block: u32,
    to_block: u32,
) -> Result<Vec<(BlockDetails, Events<AlephConfig>)>, AzeroListenerError> {
    let mut event_fetching_tasks = JoinSet::new();

    for block_number in from_block..=to_block {
        let azero_connection = azero_connection.clone();

        event_fetching_tasks.spawn(async move {
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

            Ok::<_, AzeroListenerError>((
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
