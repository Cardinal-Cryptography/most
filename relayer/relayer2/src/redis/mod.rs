use std::sync::{Arc, Mutex};

use redis::{Client as RedisClient, Commands, Connection, RedisError};
use thiserror::Error;
use tokio::sync::{
    broadcast,
    mpsc::{self},
};

use crate::config::Config;

pub const ETH_LAST_BLOCK_KEY: &str = "ethereum_last_known_block_number";

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum RedisManagerError {
    #[error("redis connection error")]
    Redis(#[from] RedisError),

    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<u32>),

    #[error("channel broadcast error")]
    Broadcast(#[from] broadcast::error::SendError<u32>),

    #[error("channel receive error")]
    Receive(#[from] broadcast::error::RecvError),
}

pub struct RedisManager;

impl RedisManager {
    pub async fn run(
        config: Arc<Config>,
        next_unprocessed_block_number: broadcast::Sender<u32>,
        mut last_processed_block_number: broadcast::Receiver<u32>,
    ) -> Result<(), RedisManagerError> {
        let Config {
            redis_node,
            name,
            default_sync_from_block_eth,
            ..
        } = &*config;

        let client = RedisClient::open(redis_node.clone())?;
        let redis_connection = Arc::new(Mutex::new(client.get_connection()?));

        let first_unprocessed_block_number = read_first_unprocessed_block_number(
            name.clone(),
            ETH_LAST_BLOCK_KEY.to_string(),
            Arc::clone(&redis_connection),
            **default_sync_from_block_eth,
        );

        _ = next_unprocessed_block_number.send(first_unprocessed_block_number)?;

        loop {
            let last_processed_block_number = last_processed_block_number.recv().await?;

            // Cache the last processed block number
            write_last_processed_block(
                name.clone(),
                ETH_LAST_BLOCK_KEY.to_string(),
                Arc::clone(&redis_connection),
                last_processed_block_number,
            )?;
        }

        // Ok(())
    }
}

pub fn read_first_unprocessed_block_number(
    name: String,
    key: String,
    redis_connection: Arc<Mutex<Connection>>,
    default_block: u32,
) -> u32 {
    let mut locked_connection = redis_connection.lock().expect("mutex lock");

    match locked_connection.get::<_, u32>(format!("{name}:{key}")) {
        Ok(value) => value + 1,
        Err(why) => {
            log::warn!("Redis connection error {why:?}");
            default_block
        }
    }
}

pub fn write_last_processed_block(
    name: String,
    key: String,
    redis_connection: Arc<Mutex<Connection>>,
    last_block_number: u32,
) -> Result<(), RedisError> {
    let mut locked_connection = redis_connection.lock().expect("mutex lock");
    locked_connection.set(format!("{name}:{key}"), last_block_number)?;
    Ok(())
}
