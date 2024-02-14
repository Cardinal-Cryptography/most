use std::{
    process,
    sync::{atomic::AtomicBool, Arc},
};

use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use listeners::AdvisoryListenerError;
use log::{debug, error, info};
use redis::{Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinSet};

use crate::{
    connections::{
        azero::{self, AzeroConnectionWithSigner},
        eth,
    },
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

    #[error("advisory listener error")]
    Advisory(#[from] AdvisoryListenerError),
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::parse());
    env_logger::init();

    info!("{:#?}", &config);

    let mut tasks = JoinSet::new();
    let emergency = Arc::new(AtomicBool::new(false));

    let client = RedisClient::open(config.redis_node.clone())?;
    let redis_connection = Arc::new(Mutex::new(client.get_async_connection().await?));

    let azero_connection = Arc::new(azero::init(&config.azero_node_wss_url).await);
    let azero_signed_connection = if let Some(cid) = config.signer_cid {
        AzeroConnectionWithSigner::with_signer(
            azero::init(&config.azero_node_wss_url).await,
            cid,
            config.signer_port,
        )
        .await?
    } else if config.dev {
        let azero_seed = "//".to_owned() + &config.dev_account_index.to_string();
        let keypair = aleph_client::keypair_from_string(&azero_seed);
        AzeroConnectionWithSigner::with_keypair(
            azero::init(&config.azero_node_wss_url).await,
            keypair,
        )
    } else {
        panic!("Use dev mode or connect to a signer");
    };

    debug!("Established connection to Aleph Zero node");

    let advisory_config_rc = Arc::clone(&config);
    let advisory_emergency_rc = Arc::clone(&emergency);
    let advisory_listener_azero_connection_rc = azero_connection.clone();

    // run task only if address passed on CLI
    if config.advisory_contract_addresses.is_some() {
        tasks.spawn(async move {
            AdvisoryListener::run(
                advisory_config_rc,
                advisory_listener_azero_connection_rc,
                advisory_emergency_rc,
            )
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

    let eth_listener_config_rc = Arc::clone(&config);
    let eth_listener_eth_connection_rc = Arc::clone(&eth_connection);
    let eth_listener_redis_connection_rc = Arc::clone(&redis_connection);
    let eth_listener_emergency_rc = Arc::clone(&emergency);

    info!("Starting Ethereum listener");

    tasks.spawn(async move {
        EthListener::run(
            eth_listener_config_rc,
            azero_signed_connection,
            eth_listener_eth_connection_rc,
            eth_listener_redis_connection_rc,
            eth_listener_emergency_rc,
        )
        .await
        .map_err(ListenerError::from)
    });

    info!("Starting AlephZero listener");

    let aleph_zero_listener_config_rc = Arc::clone(&config);
    let aleph_zero_listener_azero_signed_connection_rc = azero_connection.clone();
    let aleph_zero_listener_eth_connection_rc = Arc::clone(&eth_connection);
    let aleph_zero_listener_redis_connection_rc = Arc::clone(&redis_connection);
    let aleph_zero_listener_emergency_rc = Arc::clone(&emergency);

    tasks.spawn(async move {
        AlephZeroListener::run(
            aleph_zero_listener_config_rc,
            aleph_zero_listener_azero_signed_connection_rc,
            aleph_zero_listener_eth_connection_rc,
            aleph_zero_listener_redis_connection_rc,
            aleph_zero_listener_emergency_rc,
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
