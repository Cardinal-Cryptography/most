use std::{env, process, sync::Arc};

use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{LocalWallet, WalletError};
use eyre::Result;
use log::{error, info};
use redis::Client as RedisClient;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::Mutex};

use crate::{
    connections::{azero, eth},
    listeners::{
        AlephZeroListener, AlephZeroPastEventsListener, AzeroListenerError, EthListener,
        EthListenerError, EthPastEventsListener,
    },
};

mod config;
mod connections;
mod contracts;
mod helpers;
mod listeners;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum ListenerError {
    #[error("eth listener error")]
    Eth(#[from] EthListenerError),

    #[error("eth provider connection error")]
    EthConnection(#[from] EthConnectionError),

    #[error("eth wallet error")]
    EthWallet(#[from] WalletError),

    #[error("eth listener error")]
    Azero(#[from] AzeroListenerError),
}

fn main() -> Result<()> {
    let config = Arc::new(Config::parse());

    env::set_var("RUST_LOG", config.rust_log.as_str());
    env_logger::init();

    info!("{:#?}", &config);

    let rt = Runtime::new()?;

    rt.block_on(async {
        let mut tasks = Vec::with_capacity(4);

        let client = RedisClient::open(config.redis_node.clone())
            .expect("Cannot connect to the redis cluster instance");
        let redis_connection = Arc::new(Mutex::new(client.get_async_connection().await.unwrap()));

        let keypair = aleph_client::keypair_from_string(&config.azero_sudo_seed);

        let azero_connection = Arc::new(azero::sign(
            &azero::init(&config.azero_node_wss_url).await,
            &keypair,
        ));

        let wallet =
            LocalWallet::decrypt_keystore(&config.eth_keystore_path, &config.eth_keystore_password)
                .expect("Cannot decrypt eth wallet");

        let eth_connection = Arc::new(
            eth::sign(
                eth::init(&config.eth_node_wss_url)
                    .await
                    .expect("Connection could not be made"),
                wallet,
            )
            .await
            .expect("Cannot sign the connection"),
        );

        let config_rc1 = Arc::clone(&config);
        let azero_connection_rc1 = Arc::clone(&azero_connection);
        let eth_connection_rc1 = Arc::clone(&eth_connection);
        let redis_connection_rc1 = Arc::clone(&redis_connection);

        tasks.push(tokio::spawn(async {
            EthPastEventsListener::run(
                config_rc1,
                azero_connection_rc1,
                eth_connection_rc1,
                redis_connection_rc1,
            )
            .await
            .expect("Ethereum past events listener task has failed")
        }));

        let config_rc2 = Arc::clone(&config);
        let azero_connection_rc2 = Arc::clone(&azero_connection);
        let eth_connection_rc2 = Arc::clone(&eth_connection);
        let redis_connection_rc2 = Arc::clone(&redis_connection);

        tasks.push(tokio::spawn(async {
            EthListener::run(
                config_rc2,
                azero_connection_rc2,
                eth_connection_rc2,
                redis_connection_rc2,
            )
            .await
            .expect("Ethereum listener task has failed")
        }));

        let config_rc3 = Arc::clone(&config);
        let azero_connection_rc3 = Arc::clone(&azero_connection);
        let eth_connection_rc3 = Arc::clone(&eth_connection);
        let redis_connection_rc3 = Arc::clone(&redis_connection);

        tasks.push(tokio::spawn(async {
            AlephZeroPastEventsListener::run(
                config_rc3,
                azero_connection_rc3,
                eth_connection_rc3,
                redis_connection_rc3,
            )
            .await
            .expect("AlephZero past events listener task has failed")
        }));

        let config_rc4 = Arc::clone(&config);
        let azero_connection_rc4 = Arc::clone(&azero_connection);
        let eth_connection_rc4 = Arc::clone(&eth_connection);
        let redis_connection_rc4 = Arc::clone(&redis_connection);

        tasks.push(tokio::spawn(async {
            AlephZeroListener::run(
                config_rc4,
                azero_connection_rc4,
                eth_connection_rc4,
                redis_connection_rc4,
            )
            .await
            .expect("AlephZero listener task has failed")
        }));

        for t in tasks {
            t.await.expect("task failure");
        }
    });

    process::exit(-1);
}
