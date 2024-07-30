use std::{
    cmp::min,
    sync::Arc,
    time::{Duration, Instant},
};

use aleph_client::AccountId;
use clap::Parser;
use config::Config;
use connections::{
    azero::{AzeroConnectionWithSigner, AzeroWsConnection},
    eth::{EthConnection, EthConnectionError, GasEscalatingEthConnection, SignedEthConnection},
};
use ethers::signers::{coins_bip39::English, MnemonicBuilder, Signer};
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
    connections::{
        azero,
        eth::{self, with_gas_escalator},
    },
    contracts::MostInstance,
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

/// minimum amount of time the relayer should run healthy to reset the backoff duration to the default value
const MINIMUM_TASK_LENGHT: Duration = Duration::from_millis(600000); // 10 minutes
/// starting backoff value
const DEFAULT_BACKOFF_DURATION: Duration = Duration::from_millis(2000); // 2 seconds
/// maximal backoff value
const MAX_BACKOFF_DURATION: Duration = Duration::from_millis(600000); // 10 minutes

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
enum RelayerError {
    #[error("AlephZero node connection error")]
    AzeroConnection(#[from] connections::azero::Error),

    #[error("Ethereum node connection error")]
    EthereumConnection(#[from] connections::eth::EthConnectionError),

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
    AdvisoryEmergency(#[allow(dead_code)] Vec<AccountId>), // field is needed for logs
    AlephClientError,                                      // signifies a connection error
    EthConnectionError,
}

async fn create_azero_connections(
    config: &Config,
) -> Result<(Arc<AzeroWsConnection>, Arc<AzeroConnectionWithSigner>), connections::azero::Error> {
    let azero_connection = azero::init(&config.azero_node_wss_url).await;
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
        .await?
    } else {
        panic!("Use dev mode or connect to a signer");
    };

    Ok((
        Arc::new(azero_connection),
        Arc::new(azero_signed_connection),
    ))
}

async fn create_eth_connections(
    config: &Config,
    persistent_eth_connection: GasEscalatingEthConnection,
) -> Result<(Arc<EthConnection>, Arc<SignedEthConnection>), EthConnectionError> {
    let eth_signed_connection = if let Some(cid) = config.signer_cid {
        info!("Creating signed connection using a Signer client");
        eth::with_signer(persistent_eth_connection, cid, config.signer_port).await?
    } else if config.dev {
        let wallet =
            // use the default development mnemonic
            MnemonicBuilder::<English>::default()
                .phrase(DEV_MNEMONIC)
                .index(config.dev_account_index)?
                .build()?;

        let private_key = wallet
            .signer()
            .to_bytes()
            .iter()
            .map(|&i| format!("{:X}", i))
            .collect::<Vec<String>>()
            .join("");

        info!(
            "Creating signed connection using a development key {} [{private_key}]",
            &wallet.address()
        );
        eth::with_local_wallet(persistent_eth_connection, wallet).await?
    } else {
        panic!("Use dev mode or connect to a signer");
    };

    Ok((
        Arc::new(eth::connect(config).await),
        Arc::new(eth_signed_connection),
    ))
}

#[tokio::main]
async fn main() -> Result<(), RelayerError> {
    let config = Arc::new(Config::parse());
    env_logger::init();

    info!("{:#?}", &config);

    let mut tasks = JoinSet::new();
    let mut first_run = true;
    // Gas escalator should be shared between all relayer runs - otherwise the gas escalating task will leak on every restart
    let persistent_eth_connection = with_gas_escalator(eth::connect(&config).await).await;

    run_relayer(
        first_run,
        &mut tasks,
        config.clone(),
        persistent_eth_connection.clone(),
    )
    .await?;

    first_run = false;

    // wait for all tasks to finish and reboot
    let mut delay = DEFAULT_BACKOFF_DURATION;
    let mut tick = Instant::now();

    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(result) => {
                debug!("One of the core components exited gracefully due to : {result:?}, remaining: {}", &tasks.len());

                if tasks.is_empty() {
                    let tock = tick.elapsed();
                    info!("Relayer exited after {tock:?}. ");

                    if tock >= MINIMUM_TASK_LENGHT {
                        delay = DEFAULT_BACKOFF_DURATION;
                    } else {
                        delay = min(MAX_BACKOFF_DURATION, delay + delay / 10);
                    }
                    info!("Waiting {delay:?} before rebooting.");

                    sleep(delay).await;
                    run_relayer(
                        first_run,
                        &mut tasks,
                        config.clone(),
                        persistent_eth_connection.clone(),
                    )
                    .await?;
                    tick = Instant::now();
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

async fn run_relayer(
    first_run: bool,
    tasks: &mut JoinSet<Result<CircuitBreakerEvent, RelayerError>>,
    config: Arc<Config>,
    persistent_eth_connection: GasEscalatingEthConnection,
) -> Result<(), RelayerError> {
    // create connections
    let (azero_connection, azero_signed_connection) = create_azero_connections(&config).await?;
    info!("Established connection to Aleph Zero node");

    let (eth_connection, eth_signed_connection) =
        create_eth_connections(&config, persistent_eth_connection).await?;
    info!("Established connection to the Ethereum node");

    let most_azero = MostInstance::new(
        &config.azero_contract_address,
        &config.azero_contract_metadata,
        config.azero_ref_time_limit,
        config.azero_proof_size_limit,
    )?;

    let current_committee_id = most.current_committee_id().await?;
    most_azero
        .set_payout_account(
            azero_connection,
            current_committee_id,
            config.payout_account,
        )
        .await?;

    // Create channels
    let (eth_events_sender, eth_events_receiver) = mpsc::channel::<EthMostEvents>(1);
    let (eth_block_number_sender, _) = broadcast::channel::<u32>(1);

    let (azero_events_sender, azero_events_receiver) = mpsc::channel::<AzeroMostEvents>(32);
    let (azero_block_number_sender, azero_block_number_receiver) = broadcast::channel::<u32>(1);
    let (azero_block_seal_sender, azero_block_seal_receiver) = mpsc::channel::<u32>(1);

    let (circuit_breaker_sender, _circuit_breaker_receiver) =
        broadcast::channel::<CircuitBreakerEvent>(1);

    let advisory_addresses = Arc::new(AdvisoryListener::parse_advisory_addresses(config.clone()));

    // Check advisory status before starting the relayer
    let active_advisories = AdvisoryListener::query_active_advisories(
        advisory_addresses.clone(),
        azero_connection.clone(),
    )
    .await?;

    // If there are active advisories, we should avoid starting the relayer.
    // Starting all the components might lead to a race condition in which event handlers
    // might start processing before advisory listener activates the circuit breaker.
    if !active_advisories.is_empty() {
        info!("Active advisories detected: {active_advisories:?} - Relayer will not start.");
        tasks.spawn(async { Ok(CircuitBreakerEvent::AdvisoryEmergency(active_advisories)) });
        return Ok(());
    }

    // Receivers need to be prepared beforehand in order to receive all the data from other components
    let advisory_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let aleph_halted_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let eth_paused_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let redis_manager_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let eth_listener_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let eth_events_handler_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let aleph_listener_circuit_breaker_receiver = circuit_breaker_sender.subscribe();
    let aleph_events_handler_circuit_breaker_receiver = circuit_breaker_sender.subscribe();

    let redis_manager_eth_block_number_receiver = eth_block_number_sender.subscribe();
    let eth_listener_eth_block_number_receiver = eth_block_number_sender.subscribe();

    tasks.spawn(
        AdvisoryListener::run(
            advisory_addresses,
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
            advisory_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroHaltedListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
            aleph_halted_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumPausedListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            circuit_breaker_sender.clone(),
            eth_paused_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        RedisManager::run(
            first_run,
            Arc::clone(&config),
            eth_block_number_sender.clone(),
            redis_manager_eth_block_number_receiver,
            azero_block_number_sender.clone(),
            azero_block_seal_receiver,
            redis_manager_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            eth_events_sender.clone(),
            eth_block_number_sender.clone(),
            eth_listener_eth_block_number_receiver,
            eth_listener_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumEventsHandler::run(
            Arc::clone(&config),
            eth_events_receiver,
            Arc::clone(&azero_signed_connection),
            circuit_breaker_sender.clone(),
            eth_events_handler_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            azero_events_sender,
            azero_block_number_sender.clone(),
            azero_block_number_receiver,
            azero_block_seal_sender.clone(),
            circuit_breaker_sender.clone(),
            aleph_listener_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroEventsHandler::run(
            Arc::clone(&config),
            Arc::clone(&eth_signed_connection),
            azero_events_receiver,
            circuit_breaker_sender.clone(),
            aleph_events_handler_circuit_breaker_receiver,
        )
        .map_err(RelayerError::from),
    );

    Ok(())
}
