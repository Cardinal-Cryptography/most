use std::{env, sync::Arc};

use config::{Config, Load};
use eyre::Result;
use log::info;
use thiserror::Error;
use tokio::runtime::Runtime;

use crate::{
    connections::AzeroConnection,
    listeners::{AzeroListener, AzeroListenerError, EthListener, EthListenerError},
};

mod azero_contracts;
mod config;
mod connections;
mod eth_contracts;
mod helpers;
mod listeners;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum ListenerError {
    #[error("eth listener error error")]
    Eth(#[from] EthListenerError),

    #[error("eth listener error error")]
    Azero(#[from] AzeroListenerError),
}

fn main() -> Result<()> {
    let config = Arc::new(Config::load());

    env::set_var("RUST_LOG", &config.log_level);
    env_logger::init();

    info!("{:#?}", &config);

    let rt = Runtime::new().unwrap();

    // Spawn the root task
    rt.block_on(async {
        let mut tasks = Vec::with_capacity(2);

        let azero_connection = AzeroConnection::init(&config.azero_node_wss_url).await;

        let config_rc1 = Arc::clone(&config);
        let azero_connection_rc1 = Arc::clone(&azero_connection);
        tasks.push(tokio::spawn(async {
            EthListener::run(config_rc1, azero_connection_rc1)
                .await
                .map_err(ListenerError::Eth)
        }));

        let config_rc2 = Arc::clone(&config);
        let azero_connection_rc2 = Arc::clone(&azero_connection);
        tasks.push(tokio::spawn(async {
            AzeroListener::run(config_rc2, azero_connection_rc2)
                .await
                .map_err(ListenerError::Azero)
        }));

        for t in tasks {
            let _ = t.await.expect("Ooops!");
        }
    });

    Ok(())
}
