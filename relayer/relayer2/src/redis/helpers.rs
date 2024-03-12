use std::sync::Arc;

use redis::{AsyncCommands, RedisError};
use tokio::sync::Mutex;

use super::RedisConnection;

pub async fn read_first_unprocessed_block_number(
    name: String,
    key: String,
    redis_connection: RedisConnection,
    default_block: u32,
) -> u32 {
    // let mut connection = redis_connection.lock().await;
    let mut connection = redis_connection.lock().unwrap();
    match connection.get::<_, u32>(format!("{name}:{key}")).await {
        Ok(value) => value + 1,
        Err(why) => {
            log::warn!("Redis connection error {why:?}");
            default_block
        }
    }
}

pub async fn write_last_processed_block(
    name: String,
    key: String,
    redis_connection: RedisConnection,
    last_block_number: u32,
) -> Result<(), RedisError> {
    // let mut connection = redis_connection.lock().await;
    let mut connection = redis_connection.lock().unwrap();
    connection
        .set(format!("{name}:{key}"), last_block_number)
        .await?;
    Ok(())
}
