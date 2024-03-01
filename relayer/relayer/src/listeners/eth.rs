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
    sync::Mutex,
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::{
        azero::AzeroConnectionWithSigner,
        eth::EthConnection,
        redis_helpers::{read_first_unprocessed_block_number, write_last_processed_block},
    },
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthListenerError {
    #[error("provider error")]
    Provider(#[from] ProviderError),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Http>>),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("redis connection error")]
    Redis(#[from] RedisError),
}

pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 15;
pub const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: AzeroConnectionWithSigner,
        eth_connection: Arc<EthConnection>,
        redis_connection: Arc<Mutex<RedisConnection>>,
        emergency: Arc<AtomicBool>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            azero_contract_address,
            azero_contract_metadata,
            azero_proof_size_limit,
            azero_ref_time_limit,
            name,
            default_sync_from_block_eth,
            sync_step,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let most_eth = Most::new(address, Arc::clone(&eth_connection));
        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        let mut first_unprocessed_block_number = read_first_unprocessed_block_number(
            name.clone(),
            ETH_LAST_BLOCK_KEY.to_string(),
            redis_connection.clone(),
            **default_sync_from_block_eth,
        )
        .await;

        // Main Ethereum event loop.
        loop {
            // Query for the next unknowns finalized block number, if not present we wait for it.
            let next_finalized_block_number = get_next_finalized_block_number_eth(
                eth_connection.clone(),
                first_unprocessed_block_number,
            )
            .await;

            match emergency.load(Ordering::Relaxed) {
                true => {
                    warn!("Event handling paused due to an emergency state in one of the Advisory contracts");
                    sleep(Duration::from_secs(20)).await;
                }
                false => {
                    // Don't query for more than `sync_step` blocks at one time.
                    let to_block = std::cmp::min(
                        next_finalized_block_number,
                        first_unprocessed_block_number + sync_step - 1,
                    );

                    info!(
                        "Processing events from blocks {} - {}",
                        first_unprocessed_block_number, to_block
                    );

                    // Query for events.
                    let events = most_eth
                        .events()
                        .from_block(first_unprocessed_block_number)
                        .to_block(to_block)
                        .query()
                        .await?;

                    // Handle events.
                    for event in events {
                        // In case of the halt, we want to retry the event handling after the halt is resolved.
                        loop {
                            match handle_event(&event, &config, &azero_connection).await {
                                Ok(_) => break,
                                Err(EthListenerError::AzeroContract(e)) => {
                                    error!("Error when handling event {event:?}: {e}");
                                    if most_azero.is_halted(&azero_connection).await? {
                                        warn!("Most contract on Aleph Zero is halted, stopping event handling");
                                        wait_until_not_halted(&most_azero, &azero_connection)
                                            .await?;
                                    } else {
                                        return Err(EthListenerError::AzeroContract(e));
                                    }
                                }
                                Err(e) => return Err(e),
                            }
                        }
                    }

                    // Update the last block number.
                    first_unprocessed_block_number = to_block + 1;

                    // Cache the last processed block number.
                    write_last_processed_block(
                        name.clone(),
                        ETH_LAST_BLOCK_KEY.to_string(),
                        redis_connection.clone(),
                        to_block,
                    )
                    .await?;
                }
            }
        }
    }
}

async fn handle_event(
    event: &MostEvents,
    config: &Config,
    azero_connection: &AzeroConnectionWithSigner,
) -> Result<(), EthListenerError> {
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
        let Config {
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        info!("handling eth contract event: {crosschain_transfer_event:?}");

        // concat bytes
        let bytes = concat_u8_arrays(vec![
            &committee_id.as_u128().to_le_bytes(),
            dest_token_address,
            &amount.as_u128().to_le_bytes(),
            dest_receiver_address,
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
        // [Audit] This may fail because of e.g. "Transaction couldn't enter the pool because of the pool limit",
        // error, perhaps also in some other rare cases (like reorg and for some reason the transaction was unable
        // to be added back to tx pool - not sure in this case).
        // Maybe we can retry a few times, and only then return errors, or try restarting only
        // the EthListener before exiting from the whole relayer with an error?
        // My point is that restarting the AZero listener generally can cause
        // many bridged transactions to be processed and resubmitted again, because of the concurrency,
        // and if the transaction costs will vary much between the chains, an attacker
        // could force receive_request resubmission on the other chain many times with
        // a much smaller cost for them compared to the cost of the resubmission for the guardian.
        contract
            .receive_request(
                azero_connection,
                request_hash,
                committee_id.as_u128(),
                *dest_token_address,
                amount.as_u128(),
                *dest_receiver_address,
                request_nonce.as_u128(),
            )
            .await?;
    }

    Ok(())
}

pub async fn get_next_finalized_block_number_eth(
    eth_connection: Arc<EthConnection>,
    not_older_than: u32,
) -> u32 {
    loop {
        match eth_connection.get_block(BlockNumber::Finalized).await {
            Ok(block) => match block {
                Some(block) => {
                    let best_finalized_block_number = block
                        .number
                        .expect("Finalized block has a number.")
                        .as_u32();
                    if best_finalized_block_number >= not_older_than {
                        return best_finalized_block_number;
                    }
                    // [Nit] To catch infinite loops in case we broke something in the future:
                    // log::debug!("Best finalized block on Ethereum {best_finalized_block_number} has too small number (waiting for at least {not_older_than}), retrying after some time")
                }
                None => {
                    warn!("No finalized block found.");
                }
            },
            Err(e) => {
                // [Audit] Potentially infinite loop - once failed, the call will likely start
                // failing again and again. Maybe try propagating the error.
                warn!("Client error when getting last finalized block: {e}");
            }
        };

        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
    }
}

async fn wait_until_not_halted(
    most_azero: &MostInstance,
    azero_connection: &AzeroConnectionWithSigner,
) -> Result<(), EthListenerError> {
    loop {
        if !most_azero.is_halted(azero_connection).await? {
            return Ok(());
        }
        sleep(Duration::from_secs(10)).await;
    }
}
