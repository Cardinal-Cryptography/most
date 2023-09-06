use azero_listener::AzeroListenerError;
use config::{Config, Load};
use eth_listener::EthListenerError;
use eyre::Result;
use log::info;
use std::env;
use std::sync::Arc;
use thiserror::Error;
use tokio::runtime::Runtime;

mod aleph_zero;
mod azero_listener;
mod config;
mod eth_listener;

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

        let config_rc1 = Arc::clone(&config);
        tasks.push(tokio::spawn(async {
            eth_listener::run(config_rc1)
                .await
                .map_err(ListenerError::Eth)
        }));

        let config_rc2 = Arc::clone(&config);
        tasks.push(tokio::spawn(async {
            azero_listener::run(config_rc2)
                .await
                .map_err(ListenerError::Azero)
        }));

        for t in tasks {
            let _ = t.await.expect("Ooops!");
        }
    });

    Ok(())
}
