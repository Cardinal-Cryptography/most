use std::sync::{Arc, Mutex};

use log::{debug, info, warn};
use redis::{Client as RedisClient, Commands, Connection, RedisError};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc},
};

use crate::{config::Config, CircuitBreakerEvent};

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
        is_first_run: bool,
        config: Arc<Config>,
        next_unprocessed_block_number_eth: broadcast::Sender<u32>,
        mut last_processed_block_number_eth: broadcast::Receiver<u32>,
        next_unprocessed_block_number_azero: broadcast::Sender<u32>,
        mut block_seal_receiver_azero: mpsc::Receiver<u32>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, RedisManagerError> {
        let Config {
            redis_node,
            name,
            default_sync_from_block_eth,
            default_sync_from_block_azero,
            override_azero_cache,
            override_eth_cache,
            ..
        } = &*config;

        let client = RedisClient::open(redis_node.clone())?;
        let redis_connection = Arc::new(Mutex::new(client.get_connection()?));

        if *override_azero_cache && is_first_run {
            write_block_number(
                config.name.clone(),
                config.redis_azero_block_key.clone(),
                redis_connection.clone(),
                *config.default_sync_from_block_azero,
            )?;
        }

        if *override_eth_cache && is_first_run {
            write_block_number(
                config.name.clone(),
                config.redis_eth_block_key.clone(),
                redis_connection.clone(),
                *config.default_sync_from_block_eth,
            )?;
        }

        let first_unprocessed_block_number_eth = read_block_number(
            name.clone(),
            config.redis_eth_block_key.clone(),
            Arc::clone(&redis_connection),
            **default_sync_from_block_eth,
        );

        next_unprocessed_block_number_eth.send(first_unprocessed_block_number_eth)?;

        let first_unprocessed_block_number_azero = read_block_number(
            name.clone(),
            config.redis_azero_block_key.clone(),
            Arc::clone(&redis_connection),
            **default_sync_from_block_azero,
        );

        next_unprocessed_block_number_azero.send(first_unprocessed_block_number_azero)?;

        info!("Starting");

        loop {
            debug!("Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                Ok (last_processed_block_number) = last_processed_block_number_eth.recv() => {

                    info!("Caching {last_processed_block_number} block number for ethereum");

                    write_block_number(
                        name.clone(),
                        config.redis_eth_block_key.clone(),
                        Arc::clone(&redis_connection),
                        last_processed_block_number
                    )?;
                },

                Some (seal_block_number) = block_seal_receiver_azero.recv () => {

                    info!("Caching {seal_block_number} block number for AlephZero");

                    write_block_number(
                        name.clone(),
                        config.redis_azero_block_key.clone(),
                        Arc::clone(&redis_connection),
                        seal_block_number + 1,
                    )?;
                }

            }
        }
    }
}

pub fn read_block_number(
    name: String,
    key: String,
    redis_connection: Arc<Mutex<Connection>>,
    default_block: u32,
) -> u32 {
    let mut locked_connection = redis_connection.lock().expect("mutex lock");

    match locked_connection.get::<_, u32>(format!("{name}:{key}")) {
        Ok(value) => value,
        Err(why) => {
            log::warn!("Redis connection error {why:?}");
            default_block
        }
    }
}

/// Caches the last processed block number
pub fn write_block_number(
    name: String,
    key: String,
    redis_connection: Arc<Mutex<Connection>>,
    last_block_number: u32,
) -> Result<(), RedisError> {
    let mut locked_connection = redis_connection.lock().expect("mutex lock");
    locked_connection.set(format!("{name}:{key}"), last_block_number)?;
    Ok(())
}
