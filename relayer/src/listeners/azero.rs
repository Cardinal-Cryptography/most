use std::{collections::HashMap, sync::Arc};

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    contract_transcode::Value,
    utility::BlocksApi,
    AlephConfig, AsConnection,
};
use ethers::{
    abi::{self, EncodePackedError, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::ProviderError,
    types::U256,
    utils::keccak256,
};
use futures::StreamExt;
use log::{debug, info, trace, warn};
use redis::{aio::Connection as RedisConnection, AsyncCommands, RedisError};
use subxt::{events::Events, utils::H256};
use thiserror::Error;
use tokio::sync::Mutex;

use crate::{
    config::Config,
    connections::{
        azero::SignedAzeroWsConnection,
        eth::{EthConnectionError, EthWsConnection, SignedEthWsConnection},
    },
    contracts::{AzeroContractError, Most, MostInstance},
    helpers::chunks,
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

    #[error("missing data from event")]
    MissingEventData(String),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("redis connection error")]
    Redis(#[from] RedisError),

    #[error("unexpected error")]
    Unexpected,
}

pub struct AlephZeroPastEventsListener;

impl AlephZeroPastEventsListener {
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
            sync_step,
            default_sync_from_block,
            ..
        } = &*config;

        let mut connection = redis_connection.lock().await;

        let last_known_block_number: u32 = match connection
            .get(format!("{name}:{ALEPH_LAST_BLOCK_KEY}"))
            .await
        {
            Ok(value) => value,
            Err(why) => {
                warn!("Redis connection error {why:?}");
                *default_sync_from_block
            }
        };

        // replay past events from last known to the latest
        let last_block_number = azero_connection
            .get_block_number_opt(None)
            .await?
            .ok_or(AzeroListenerError::BlockNotFound)?;

        let instance = MostInstance::new(azero_contract_address, azero_contract_metadata)?;
        let contracts = vec![&instance.contract];

        for (from, to) in chunks(last_known_block_number, last_block_number, *sync_step) {
            for block_number in from..to {
                let block_hash = azero_connection
                    .get_block_hash(block_number)
                    .await?
                    .ok_or(AzeroListenerError::BlockNotFound)?;

                let connection = azero_connection.as_connection();
                let events = connection
                    .as_client()
                    .blocks()
                    .at(block_hash)
                    .await?
                    .events()
                    .await?;

                // filter contract events
                handle_events(
                    Arc::clone(&eth_connection),
                    &config,
                    events,
                    &contracts,
                    block_number,
                    block_hash,
                )
                .await?;
            }
        }

        info!("finished processing past events");

        Ok(())
    }
}

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
            ..
        } = &*config;

        let instance = MostInstance::new(azero_contract_address, azero_contract_metadata)?;

        let contracts = vec![&instance.contract];

        // subscribe to new events
        let connection = azero_connection.as_connection();
        let mut subscription = connection
            .as_client()
            .blocks()
            .subscribe_finalized()
            .await?;

        info!("subscribing to new events");

        while let Some(Ok(block)) = subscription.next().await {
            let block_number = block.number();

            let events = block.events().await?;
            handle_events(
                Arc::clone(&eth_connection),
                &config,
                events,
                &contracts,
                block_number,
                block.hash(),
            )
            .await?;

            let mut connection = redis_connection.lock().await;
            connection
                .set(format!("{name}:{ALEPH_LAST_BLOCK_KEY}"), block_number)
                .await?;

            info!("persisted last_block_number: {block_number}");
        }

        Ok(())
    }
}

async fn handle_events(
    eth_connection: Arc<SignedEthWsConnection>,
    config: &Config,
    events: Events<AlephConfig>,
    contracts: &[&ContractInstance],
    block_number: u32,
    block_hash: H256,
) -> Result<(), AzeroListenerError> {
    for event in translate_events(
        events.iter(),
        contracts,
        Some(BlockDetails {
            block_number,
            block_hash,
        }),
    ) {
        handle_event(Arc::clone(&eth_connection), config, event?).await?;
    }
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
    config: &Config,
    event: ContractEvent,
) -> Result<(), AzeroListenerError> {
    let Config {
        eth_contract_address,
        ..
    } = config;

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
            let contract = Most::new(address, eth_connection);

            //  forward transfer & vote
            let call: ContractCall<SignedEthWsConnection, ()> = contract.receive_request(
                request_hash,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
            );

            let tx = call
                .send()
                .await?
                .await?
                .ok_or(AzeroListenerError::NoTxReceipt)?;

            info!("eth tx confirmed: {tx:?}");
        }
    }
    Ok(())
}
