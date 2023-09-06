use crate::config::Config;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {}

pub async fn run(config: Arc<Config>) -> Result<(), AzeroListenerError> {
    println!("@azero listener");

    // todo!("")
    Ok(())
}
