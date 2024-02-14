use std::sync::{atomic::AtomicBool, Arc};

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::{k256::ecdsa::SigningKey, ContractError, SignerMiddleware},
    providers::{Middleware, ProviderError},
    signers::Wallet,
    types::BlockNumber,
    utils::keccak256,
};
use log::{debug, info, trace, warn};
use redis::{aio::Connection as RedisConnection, RedisError};
use thiserror::Error;
use tokio::{
    sync::Mutex,
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::{
        azero::SignedAzeroWsConnection,
        eth::{EthConnection, SignedEthConnection},
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
    Contract(#[from] ContractError<SignerMiddleware<EthConnection, Wallet<SigningKey>>>),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("redis connection error")]
    Redis(#[from] RedisError),
}

pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 15;
const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthConnection>,
        redis_connection: Arc<Mutex<RedisConnection>>,
        emergency: Arc<AtomicBool>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            name,
            default_sync_from_block_eth,
            sync_step,
            override_eth_cache,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Most::new(address, Arc::clone(&eth_connection));

        let mut first_unprocessed_block_number = if *override_eth_cache {
            **default_sync_from_block_eth
        } else {
            read_first_unprocessed_block_number(
                name.clone(),
                ETH_LAST_BLOCK_KEY.to_string(),
                redis_connection.clone(),
                **default_sync_from_block_eth,
            )
            .await
        };

        // Main Ethereum event loop.
        loop {
            // Query for the next unknowns finalized block number, if not present we wait for it.
            let next_finalized_block_number = get_next_finalized_block_number_eth(
                eth_connection.clone(),
                first_unprocessed_block_number,
            )
            .await;

            match emergency.load(std::sync::atomic::Ordering::Relaxed) {
                true => trace!("Event handling paused due to an emergency state in one of the Advisory contracts"),
                false => {

            // Don't query for more than `sync_step` blocks at one time.
            let to_block = std::cmp::min(
                next_finalized_block_number,
                first_unprocessed_block_number + sync_step - 1,
            );

            log::info!(
                "Processing events from blocks {} - {}",
                first_unprocessed_block_number,
                to_block
            );

            // Query for events.
            let events = contract
                .events()
                .from_block(first_unprocessed_block_number)
                .to_block(to_block)
                .query()
                .await?;

            // Handle events.
            for event in events {
                handle_event(
                    &event,
                    &config,
                    Arc::clone(&azero_connection),
                )
                .await?
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
                },
            }
        }
    }
}

async fn handle_event(
    event: &MostEvents,
    config: &Config,
    azero_connection: Arc<SignedAzeroWsConnection>,
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
            relayers_committee_id,
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        if *relayers_committee_id != committee_id.as_u128() {
            warn!(
                "Ignoring event from committee {}, expected {}",
                committee_id, relayers_committee_id
            );
            return Ok(());
        }

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
        contract
            .receive_request(
                &azero_connection,
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
    eth_connection: Arc<SignedEthConnection>,
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
                }
                None => {
                    warn!("No finalized block found.");
                }
            },
            Err(e) => {
                warn!("Client error when getting last finalized block: {e}");
            }
        };

        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
    }
}
