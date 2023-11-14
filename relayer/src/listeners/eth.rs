use std::sync::Arc;

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::{k256::ecdsa::SigningKey, ContractError, SignerMiddleware},
    providers::{Middleware, Provider, ProviderError, Ws},
    signers::Wallet,
    types::{BlockNumber},
    utils::keccak256,
};
use log::{debug, info, trace, warn};
use redis::{aio::Connection as RedisConnection, AsyncCommands, RedisError};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    connections::{azero::SignedAzeroWsConnection, eth::SignedEthWsConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Membrane, MembraneEvents,
        MembraneInstance,
    },
    helpers::{concat_u8_arrays},
};

const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthListenerError {
    #[error("provider error")]
    Provider(#[from] ProviderError),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<SignerMiddleware<Provider<Ws>, Wallet<SigningKey>>>),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("redis connection error")]
    Redis(#[from] RedisError),
}

const BLOCK_PROD_TIME_SEC: u32 = 15;

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
        redis_connection: Arc<Mutex<RedisConnection>>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            name,
            default_sync_from_block_eth,
            sync_step,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Membrane::new(address, Arc::clone(&eth_connection));

        let mut first_unprocessed_block_number = read_first_unprocessed_block_number(
            name.clone(),
            redis_connection.clone(),
            *default_sync_from_block_eth,
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
                handle_event(&event, &config, Arc::clone(&azero_connection)).await?
            }

            // Update the last block number.
            first_unprocessed_block_number = to_block + 1;

            // Cache the last processed block number.
            write_last_processed_block(name.clone(), redis_connection.clone(), to_block).await?;
        }
    }
}

async fn handle_event(
    event: &MembraneEvents,
    config: &Config,
    azero_connection: Arc<SignedAzeroWsConnection>,
) -> Result<(), EthListenerError> {
    if let MembraneEvents::CrosschainTransferRequestFilter(
        crosschain_transfer_event @ CrosschainTransferRequestFilter {
            dest_token_address,
            amount,
            dest_receiver_address,
            request_nonce,
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
            dest_token_address,
            &amount.as_u128().to_le_bytes(),
            dest_receiver_address,
            &request_nonce.as_u128().to_le_bytes(),
        ]);

        trace!("event concatenated bytes: {bytes:?}");

        let request_hash = keccak256(bytes);
        debug!("hashed event encoding: {request_hash:?}");

        let contract = MembraneInstance::new(azero_contract_address, azero_contract_metadata)?;

        // send vote
        contract
            .receive_request(
                &azero_connection,
                request_hash,
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
    eth_connection: Arc<SignedEthWsConnection>,
    not_older_than: u32,
) -> u32 {
    let mut best_finalized_block_number_opt: Option<u32>;
    loop {
        let finalized_block_opt = match eth_connection.get_block(BlockNumber::Finalized).await {
            Ok(block) => Some(block.expect("msg")),
            Err(e) => {
                warn!("Client error when getting last finalized block: {e}");
                None
            }
        };

        best_finalized_block_number_opt = finalized_block_opt.clone().map(|block| {
            block
                .number
                .expect("Finalized block should have a number.")
                .as_u32()
        });

        if let Some(best_finalized_block_number) = best_finalized_block_number_opt {
            if best_finalized_block_number >= not_older_than {
                break;
            }    
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(BLOCK_PROD_TIME_SEC.into())).await;
    }
    best_finalized_block_number_opt.expect("We return only if we managed to fetch the block.")
}

async fn read_first_unprocessed_block_number(
    name: String,
    redis_connection: Arc<Mutex<RedisConnection>>,
    default_block: u32,
) -> u32 {
    let mut connection = redis_connection.lock().await;

    match connection.get::<_, u32>(format!("{name}:{ETH_LAST_BLOCK_KEY}")).await {
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
) -> Result<(), EthListenerError> {
    let mut connection = redis_connection.lock().await;
    connection
        .set(format!("{name}:{ETH_LAST_BLOCK_KEY}"), last_block_number)
        .await?;
    Ok(())
}
