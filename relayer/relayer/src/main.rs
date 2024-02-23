use std::{
    process,
    sync::{atomic::AtomicBool, Arc},
};


use aleph_client::Connection;
use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use listeners::AdvisoryListenerError;
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinSet};

use crate::{
    connections::{
        azero::{self, AzeroConnectionWithSigner},
        eth,
        redis_helpers::write_last_processed_block,
    },
    eth::{EthConnection, SignedEthConnection},
    listeners::{
        AdvisoryListener, AlephZeroListener, AzeroListenerError, EthListener, EthListenerError,
        ALEPH_LAST_BLOCK_KEY, ETH_LAST_BLOCK_KEY,
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

    let client = RedisClient::open(config.redis_node.clone())?;
    let redis_connection = Arc::new(Mutex::new(client.get_async_connection().await?));

    if config.override_azero_cache {
        write_last_processed_block(
            config.name.clone(),
            ALEPH_LAST_BLOCK_KEY.to_string(),
            redis_connection.clone(),
            *config.default_sync_from_block_azero - 1,
        )
        .await?;
    }

    if config.override_eth_cache {
        write_last_processed_block(
            config.name.clone(),
            ETH_LAST_BLOCK_KEY.to_string(),
            redis_connection.clone(),
            *config.default_sync_from_block_eth - 1,
        )
        .await?;
    }

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

    let eth_signed_connection = if let Some(cid) = config.signer_cid {
        eth::with_signer(
            eth::connect(&config.eth_node_http_url).await,
            cid,
            config.signer_port,
        )
        .await?
    } else {
        eth::with_local_wallet(eth::connect(&config.eth_node_http_url).await, wallet).await?
    };
    let eth_signed_connection = Arc::new(eth_signed_connection);

    let eth_connection = Arc::new(eth::connect(&config.eth_node_http_url).await);

    debug!("Established connection to Ethereum node");

    if let Err(err) = run_listeners(
        config,
        azero_connection,
        azero_signed_connection,
        eth_connection,
        eth_signed_connection,
        redis_connection,
    )
    .await
    {
        error!(
            "Error when running listeners, this might require manual investigation or RESTART..."
        );
        err.chain().enumerate().for_each(|(level, cause)| {
            let cause = cause.to_string();
            if cause.len() > 100 {
                error!(" {}: {}...", level, &cause[..100]);
            } else {
                error!(" {}: {}", level, cause);
            }
        });
    }

    process::exit(-1);
}

async fn run_listeners(
    config: Arc<Config>,
    azero_connection: Arc<Connection>,
    azero_signed_connection: AzeroConnectionWithSigner,
    eth_connection: Arc<EthConnection>,
    eth_signed_connection: Arc<SignedEthConnection>,
    redis_connection: Arc<Mutex<RedisConnection>>,
) -> Result<()> {
    let mut tasks = JoinSet::new();
    let emergency = Arc::new(AtomicBool::new(false));

    // run task only if address passed on CLI
    if config.advisory_contract_addresses.is_some() {
        spawn_advisory_listener(
            &mut tasks,
            config.clone(),
            azero_connection.clone(),
            emergency.clone(),
        );
    }

    spawn_eth_listener(
        &mut tasks,
        config.clone(),
        azero_signed_connection,
        eth_connection.clone(),
        redis_connection.clone(),
        emergency.clone(),
    );

    spawn_azero_listener(
        &mut tasks,
        config.clone(),
        azero_connection.clone(),
        eth_signed_connection.clone(),
        redis_connection.clone(),
        emergency.clone(),
    );

    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(_) => {}
            Err(ListenerError::Azero(AzeroListenerError::BridgeHaltedRestartRequired)) => {
                warn!("Restarting AlephZero listener");
                spawn_azero_listener(
                    &mut tasks,
                    config.clone(),
                    azero_connection.clone(),
                    eth_signed_connection.clone(),
                    redis_connection.clone(),
                    emergency.clone(),
                );
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn spawn_azero_listener(
    tasks: &mut JoinSet<Result<(), ListenerError>>,
    config: Arc<Config>,
    azero_connection: Arc<Connection>,
    eth_signed_connection: Arc<SignedEthConnection>,
    redis_connection: Arc<Mutex<RedisConnection>>,
    emergency: Arc<AtomicBool>,
) {
    info!("Starting AlephZero listener");
    tasks.spawn(async move {
        AlephZeroListener::run(
            config,
            azero_connection,
            eth_signed_connection,
            redis_connection,
            emergency,
        )
        .await
        .map_err(ListenerError::from)
    });
}

fn spawn_eth_listener(
    tasks: &mut JoinSet<Result<(), ListenerError>>,
    config: Arc<Config>,
    azero_signed_connection: AzeroConnectionWithSigner,
    eth_connection: Arc<EthConnection>,
    redis_connection: Arc<Mutex<RedisConnection>>,
    emergency: Arc<AtomicBool>,
) {
    info!("Starting Ethereum listener");
    tasks.spawn(async move {
        EthListener::run(
            config,
            azero_signed_connection,
            eth_connection,
            redis_connection,
            emergency,
        )
        .await
        .map_err(ListenerError::from)
    });
}

fn spawn_advisory_listener(
    tasks: &mut JoinSet<Result<(), ListenerError>>,
    config: Arc<Config>,
    azero_connection: Arc<Connection>,
    emergency: Arc<AtomicBool>,
) {
    info!("Starting Advisory listener");
    tasks.spawn(async move {
        AdvisoryListener::run(config, azero_connection, emergency)
            .await
            .map_err(ListenerError::from)
    });
}
