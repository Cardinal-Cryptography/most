use std::{
    env, process,
    sync::{atomic::AtomicBool, Arc},
};

use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use log::{debug, error, info};
use redis::{Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinSet};

use crate::{
    connections::{azero, eth},
    listeners::{
        AdvisoryListener, AlephZeroListener, AzeroListenerError, EthListener, EthListenerError,
    },
};

mod config;
mod connections;
mod contracts;
mod helpers;
mod listeners;

const DEV_MNEMONIC: &str =
    "harsh master island dirt equip search awesome double turn crush wool grant";

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

    #[error("azero listener error")]
    Azero(#[from] AzeroListenerError),

    #[error("redis error")]
    Redis(#[from] RedisError),
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::parse());

    env::set_var("RUST_LOG", config.rust_log.as_str());
    env_logger::init();

    info!("{:#?}", &config);

    let mut tasks = JoinSet::new();
    let emergency = Arc::new(AtomicBool::new(false));

    let client = RedisClient::open(config.redis_node.clone())?;
    let redis_connection = Arc::new(Mutex::new(client.get_async_connection().await?));

    let azero_keypair = if config.dev {
        let azero_seed = "//".to_owned() + &config.dev_account_index.to_string();
        aleph_client::keypair_from_string(&azero_seed)
    } else {
        unimplemented!("Only dev mode is supported for now");
    };

    let azero_connection = Arc::new(azero::init(&config.azero_node_wss_url).await);
    let azero_signed_connection = Arc::new(azero::sign(&azero_connection, &azero_keypair));

    debug!("Established connection to Aleph Zero node");

    let config_rc1 = Arc::clone(&config);
    let emergency_rc1 = Arc::clone(&emergency);

    // run task only if address passed on CLI
    if config.advisory_contract_address.is_some() {
        tasks.spawn(async move {
            AdvisoryListener::run(config_rc1, azero_connection, emergency_rc1)
                .await
                .map_err(ListenerError::from)
        });
    }

    let wallet = if config.dev {
        // If no keystore path is provided, we use the default development mnemonic
        MnemonicBuilder::<English>::default()
            .phrase(DEV_MNEMONIC)
            .index(config.dev_account_index)?
            .build()?
    } else {
        info!(
            "Creating wallet from a keystore path: {}",
            config.eth_keystore_path
        );
        LocalWallet::decrypt_keystore(&config.eth_keystore_path, &config.eth_keystore_password)?
    };

    info!("Wallet address: {}", wallet.address());

    let eth_connection =
        Arc::new(eth::sign(eth::connect(&config.eth_node_http_url).await, wallet).await?);

    debug!("Established connection to Ethereum node");

    let config_rc2 = Arc::clone(&config);
    let azero_signed_connection_rc1 = Arc::clone(&azero_signed_connection);
    let eth_connection_rc1 = Arc::clone(&eth_connection);
    let redis_connection_rc1 = Arc::clone(&redis_connection);
    let emergency_rc2 = Arc::clone(&emergency);

    info!("Starting Ethereum listener");

    tasks.spawn(async move {
        EthListener::run(
            config_rc2,
            azero_signed_connection_rc1,
            eth_connection_rc1,
            redis_connection_rc1,
            emergency_rc2,
        )
        .await
        .map_err(ListenerError::from)
    });

    info!("Starting AlephZero listener");

    let config_rc2 = Arc::clone(&config);
    let azero_signed_connection_rc2 = Arc::clone(&azero_signed_connection);
    let eth_connection_rc2 = Arc::clone(&eth_connection);
    let redis_connection_rc2 = Arc::clone(&redis_connection);
    let emergency_rc3 = Arc::clone(&emergency);

    tasks.spawn(async move {
        AlephZeroListener::run(
            config_rc2,
            azero_signed_connection_rc2,
            eth_connection_rc2,
            redis_connection_rc2,
            emergency_rc3,
        )
        .await
        .map_err(ListenerError::from)
    });

    while let Some(result) = tasks.join_next().await {
        error!("Listener task has finished unexpectedly: {:?}", result);
        result??;
    }

    process::exit(-1);
}
