use std::sync::{Arc, Mutex};

use redis::{aio::Connection, Client, RedisError};
use thiserror::Error;
use tokio::sync::mpsc::{self, error::SendError};

use self::helpers::{read_first_unprocessed_block_number, write_last_processed_block};
use crate::config::Config;

mod helpers;

pub const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

pub type RedisConnection = Arc<Mutex<Connection>>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum RedisManagerError {
    #[error("redis connection error")]
    Redis(#[from] RedisError),

    #[error("channel send error")]
    Send(#[from] SendError<u32>),
}

pub struct RedisManager;

impl RedisManager {
    pub async fn create_connection(
        config: Arc<Config>,
    ) -> Result<RedisConnection, RedisManagerError> {
        let client = Client::open(config.redis_node.clone())?;
        Ok(Arc::new(Mutex::new(client.get_async_connection().await?)))
    }

    pub async fn run(
        config: Arc<Config>,
        redis_connection: RedisConnection,
        next_unprocessed_block_number: mpsc::Sender<u32>,
        mut last_processed_block_number: mpsc::Receiver<u32>,
    ) -> Result<(), RedisManagerError> {
        let Config {
            eth_contract_address,
            // azero_contract_address,
            // azero_contract_metadata,
            // azero_proof_size_limit,
            // azero_ref_time_limit,
            name,
            default_sync_from_block_eth,
            sync_step,
            ..
        } = &*config;

        let first_unprocessed_block_number = read_first_unprocessed_block_number(
            name.clone(),
            ETH_LAST_BLOCK_KEY.to_string(),
            redis_connection.clone(),
            **default_sync_from_block_eth,
        )
        .await;

        next_unprocessed_block_number
            .send(first_unprocessed_block_number)
            .await?;

        while let Some(last_processed_block_number) = last_processed_block_number.recv().await {
            // Cache the last processed block number.
            write_last_processed_block(
                name.clone(),
                ETH_LAST_BLOCK_KEY.to_string(),
                redis_connection.clone(),
                last_processed_block_number,
            )
            .await?;
        }

        Ok(())
    }
}
