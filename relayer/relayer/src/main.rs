use std::{sync::Arc, time::Duration};

use aleph_client::AccountId;
use clap::Parser;
use config::Config;
use connections::{
    azero::{AzeroConnectionWithSigner, AzeroWsConnection},
    eth::{EthConnection, SignedEthConnection},
};
use ethers::signers::{coins_bip39::English, MnemonicBuilder, Signer, WalletError};
use futures::TryFutureExt;
use handlers::{AlephZeroEventsHandlerError, EthereumEventsHandlerError};
use listeners::{
    AdvisoryListenerError, AlephZeroHaltedListenerError, AlephZeroListenerError,
    EthereumListenerError, EthereumPausedListenerError,
};
use log::{debug, error, info};
use redis::RedisManagerError;
use thiserror::Error;
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::{JoinError, JoinSet},
    time::sleep,
};

use crate::{
    connections::{azero, eth},
    handlers::{AlephZeroEventsHandler, EthereumEventsHandler},
    listeners::{
        AdvisoryListener, AlephZeroHaltedListener, AlephZeroListener, AzeroMostEvents,
        EthMostEvents, EthereumListener, EthereumPausedListener,
    },
    redis::RedisManager,
};

mod config;
mod connections;
mod contracts;
mod handlers;
mod helpers;
mod listeners;
mod redis;

const DEV_MNEMONIC: &str =
    "harsh master island dirt equip search awesome double turn crush wool grant";

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
enum RelayerError {
    #[error("AlephZero node connection error")]
    AzeroConnection(#[from] connections::azero::Error),

    #[error("Ethereum node connection error")]
    EthereumConnection(#[from] connections::eth::EthConnectionError),

    #[error("Ethereum wallet error")]
    EthWallet(#[from] WalletError),

    #[error("Task join error")]
    Join(#[from] JoinError),

    #[error("circuit breaker channel send error")]
    CircuitBreakerSend(#[from] mpsc::error::SendError<CircuitBreakerEvent>),

    #[error("ack receive error")]
    AckReceive(#[from] oneshot::error::RecvError),

    #[error("Advisory listener failure")]
    AdvisoryListener(#[from] AdvisoryListenerError),

    #[error("AlephZero Most listener failure")]
    AlephZeroListener(#[from] AlephZeroListenerError),

    #[error("AlephZero events handler failure")]
    AlephZeroEventsHandler(#[from] AlephZeroEventsHandlerError),

    #[error("Ethereum Most listener failure")]
    EthereumListener(#[from] EthereumListenerError),

    #[error("Ethereum events handler failure")]
    EthereumEventsHandler(#[from] EthereumEventsHandlerError),

    #[error("Redis manager failure")]
    RedisManager(#[from] RedisManagerError),

    #[error("AlephZero Most halted listener failure")]
    AlephZeroHaltedListener(#[from] AlephZeroHaltedListenerError),

    #[error("Ethereum's Most paused listener failure")]
    EthereumPausedListener(#[from] EthereumPausedListenerError),
}

#[derive(Debug, Clone)]
enum CircuitBreakerEvent {
    EthEventHandlerFailure,
    AlephZeroEventHandlerFailure,
    BridgeHaltAlephZero,
    BridgeHaltEthereum,
    AdvisoryEmergency(#[allow(dead_code)] AccountId), // field is needed for logs
}

#[tokio::main]
async fn main() -> Result<(), RelayerError> {
    let config = Arc::new(Config::parse());
    env_logger::init();

    info!("{:#?}", &config);

    let azero_connection = Arc::new(azero::init(&config.azero_node_wss_url).await);
    let azero_signed_connection = if let Some(cid) = config.signer_cid {
        info!("[AlephZero] Creating signed connection using a Signer client");
        AzeroConnectionWithSigner::with_signer(
            azero::init(&config.azero_node_wss_url).await,
            cid,
            config.signer_port,
        )
        .await?
    } else if config.dev {
        let azero_seed = "//".to_owned() + &config.dev_account_index.to_string();
        let keypair = aleph_client::keypair_from_string(&azero_seed);

        info!(
            "Creating signed connection using a development key {}",
            keypair.account_id()
        );

        AzeroConnectionWithSigner::with_keypair(
            azero::init(&config.azero_node_wss_url).await,
            keypair,
        )
    } else {
        panic!("Use dev mode or connect to a signer");
    };
    let azero_signed_connection = Arc::new(azero_signed_connection);

    info!("Established connection to Aleph Zero node");

    let eth_signed_connection = if let Some(cid) = config.signer_cid {
        info!("Creating signed connection using a Signer client");
        eth::with_signer(
            eth::connect(&config.eth_node_http_url).await,
            cid,
            config.signer_port,
        )
        .await?
    } else if config.dev {
        let wallet =
            // use the default development mnemonic
            MnemonicBuilder::<English>::default()
                .phrase(DEV_MNEMONIC)
                .index(config.dev_account_index)?
                .build()?;

        info!(
            "Creating signed connection using a development key {}",
            &wallet.address()
        );
        eth::with_local_wallet(eth::connect(&config.eth_node_http_url).await, wallet).await?
    } else {
        panic!("Use dev mode or connect to a signer");
    };

    let eth_signed_connection = Arc::new(eth_signed_connection);

    let eth_connection = Arc::new(eth::connect(&config.eth_node_http_url).await);

    debug!("Established connection to the Ethereum node");

    let mut tasks = JoinSet::new();

    run_relayer(
        &mut tasks,
        config.clone(),
        azero_connection.clone(),
        azero_signed_connection.clone(),
        eth_connection.clone(),
        eth_signed_connection.clone(),
    );

    // wait for all tasks to finish and reboot
    let delay = Duration::from_secs(2);
    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(result) => {
                debug!("One of the core components exited gracefully due to : {result:?}, remaining: {}", &tasks.len());

                // TODO: restart with backoff
                if tasks.is_empty() {
                    info!("Relayer exited. Waiting {delay:?} before rebooting.");
                    sleep(delay).await;

                    run_relayer(
                        &mut tasks,
                        config.clone(),
                        azero_connection.clone(),
                        azero_signed_connection.clone(),
                        eth_connection.clone(),
                        eth_signed_connection.clone(),
                    );
                }
            }
            Err(why) => {
                error!("One of the core components exited with an error {why:?}. This is fatal");
                std::process::exit(1);
            }
        }
    }

    error!("We should have never gotten here!");
    std::process::exit(1);
}

fn run_relayer(
    tasks: &mut JoinSet<Result<CircuitBreakerEvent, RelayerError>>,
    config: Arc<Config>,
    azero_connection: Arc<AzeroWsConnection>,
    azero_signed_connection: Arc<AzeroConnectionWithSigner>,
    eth_connection: Arc<EthConnection>,
    eth_signed_connection: Arc<SignedEthConnection>,
) {
    // Create channels
    let (eth_events_sender, eth_events_receiver) = mpsc::channel::<EthMostEvents>(1);
    // TODO: use mpsc if we use seal block channel for eth
    let (eth_block_number_sender, _eth_block_number_receiver) = broadcast::channel::<u32>(1);
    // TODO: create block seal channel for ethereum

    let (azero_events_sender, azero_events_receiver) = mpsc::channel::<AzeroMostEvents>(32);
    // TODO: this should be mpsc
    let (azero_block_number_sender, _azero_block_number_receiver) = broadcast::channel::<u32>(1);

    let (azero_block_seal_sender, azero_block_seal_receiver) = mpsc::channel::<u32>(1);

    let (circuit_breaker_sender, _circuit_breaker_receiver) =
        broadcast::channel::<CircuitBreakerEvent>(1);

    tasks.spawn(
        AdvisoryListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroHaltedListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumPausedListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            circuit_breaker_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        RedisManager::run(
            Arc::clone(&config),
            eth_block_number_sender.clone(),
            eth_block_number_sender.subscribe(),
            azero_block_number_sender.clone(),
            azero_block_seal_receiver,
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            eth_events_sender.clone(),
            eth_block_number_sender.clone(),
            eth_block_number_sender.subscribe(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumEventsHandler::run(
            Arc::clone(&config),
            eth_events_receiver,
            Arc::clone(&azero_signed_connection),
            circuit_breaker_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            azero_events_sender,
            azero_block_number_sender.clone(),
            azero_block_number_sender.subscribe(),
            azero_block_seal_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroEventsHandler::run(
            Arc::clone(&config),
            Arc::clone(&eth_signed_connection),
            azero_events_receiver,
            azero_block_seal_sender.clone(),
            circuit_breaker_sender.clone(),
            circuit_breaker_sender.subscribe(),
        )
        .map_err(RelayerError::from),
    );
}
