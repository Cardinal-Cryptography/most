use std::sync::Arc;

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::{k256::ecdsa::SigningKey, ContractError, SignerMiddleware},
    providers::{Middleware, Provider, ProviderError, StreamExt, Ws},
    signers::Wallet,
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
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents,
        MostInstance,
    },
    helpers::{chunks, concat_u8_arrays},
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

pub struct EthPastEventsListener;

impl EthPastEventsListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
        redis_connection: Arc<Mutex<RedisConnection>>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            name,
            default_sync_from_block,
            sync_step,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Most::new(address, Arc::clone(&eth_connection));

        let mut connection = redis_connection.lock().await;

        let last_known_block_number: u32 =
            match connection.get(format!("{name}:{ETH_LAST_BLOCK_KEY}")).await {
                Ok(value) => value,
                Err(why) => {
                    warn!("Redis connection error {why:?}");
                    *default_sync_from_block
                }
            };

        let last_block_number = eth_connection.get_block_number().await.unwrap().as_u32();

        info!("retrieved last known block number: {last_known_block_number}");

        for (from, to) in chunks(last_known_block_number, last_block_number, *sync_step) {
            let past_events = contract
                .events()
                .from_block(from)
                .to_block(to)
                .query()
                .await
                .unwrap();

            for event in past_events {
                handle_event(&event, &config, Arc::clone(&azero_connection)).await?
            }
        }

        info!("finished processing past events");

        Ok(())
    }
}

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
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Most::new(address, Arc::clone(&eth_connection));

        let last_block_number = eth_connection.get_block_number().await.unwrap().as_u32();

        let events = contract.events().from_block(last_block_number);
        let mut stream = events.stream().await?.with_meta();

        info!("subscribing to new events");

        while let Some(Ok((event, meta))) = stream.next().await {
            handle_event(&event, &config, Arc::clone(&azero_connection)).await?;

            // persist the last seen block number
            let block_number = meta.block_number.as_u32();
            let mut connection = redis_connection.lock().await;
            connection
                .set(format!("{name}:{ETH_LAST_BLOCK_KEY}"), block_number)
                .await?;

            info!("persisted last known block number: {block_number}");
        }

        Ok(())
    }
}

async fn handle_event(
    event: &MostEvents,
    config: &Config,
    azero_connection: Arc<SignedAzeroWsConnection>,
) -> Result<(), EthListenerError> {
    if let MostEvents::CrosschainTransferRequestFilter(
        crosschain_transfer_event @ CrosschainTransferRequestFilter {
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
            dest_token_address,
            &amount.as_u128().to_le_bytes(),
            dest_receiver_address,
            &request_nonce.as_u128().to_le_bytes(),
        ]);

        trace!("event concatenated bytes: {bytes:?}");

        let request_hash = keccak256(bytes);
        debug!("hashed event encoding: {request_hash:?}");

        let contract = MostInstance::new(azero_contract_address, azero_contract_metadata)?;

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
