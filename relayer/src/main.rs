use std::{env, process, sync::Arc};

use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{LocalWallet, WalletError};
use eyre::Result;
use log::{error, info};
use thiserror::Error;
use tokio::runtime::Runtime;

use crate::{
    connections::{azero, eth},
    listeners::{AzeroListener, AzeroListenerError, EthListener, EthListenerError},
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
        let mut tasks = Vec::with_capacity(2);

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
        tasks.push(tokio::spawn(async {
            EthListener::run(config_rc1, azero_connection_rc1, eth_connection_rc1)
                .await
                .map_err(ListenerError::Eth)
        }));

        let config_rc2 = Arc::clone(&config);
        let azero_connection_rc2 = Arc::clone(&azero_connection);
        let eth_connection_rc2 = Arc::clone(&eth_connection);
        tasks.push(tokio::spawn(async {
            AzeroListener::run(config_rc2, azero_connection_rc2, eth_connection_rc2)
                .await
                .map_err(ListenerError::Azero)
        }));

        for t in tasks {
            let result = t.await.expect("Ooops!");
            if let Err(why) = result {
                error!("{why:?}");
            }
        }
    });

    process::exit(-1);
}
