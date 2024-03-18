use std::{cmp::max, sync::Arc};

use aleph_client::AccountId;
use clap::Parser;
use config::Config;
use connections::{azero::AzeroConnectionWithSigner, eth::SignedEthConnection};
use ethers::signers::{coins_bip39::English, MnemonicBuilder, Signer, WalletError};
use futures::TryFutureExt;
use handlers::{AlephZeroEventsHandlerError, EthereumEventsHandlerError};
use listeners::{
    AdvisoryListenerError, AlephZeroHaltedListenerError, AzeroListenerError, EthereumListenerError,
    EthereumPausedListenerError,
};
use log::{debug, error, info, warn};
use redis::RedisManagerError;
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot, Mutex},
    task::{JoinError, JoinSet},
    time::{sleep, Duration},
};

use crate::{
    connections::{azero, eth},
    handlers::{
        AlephZeroEventHandler, AlephZeroEventsHandler, EthereumEventHandler, EthereumEventsHandler,
    },
    listeners::{
        AdvisoryListener, AlephZeroHaltedListener, AlephZeroListener, AzeroMostEvent,
        AzeroMostEvents, EthMostEvent, EthMostEvents, EthereumListener, EthereumPausedListener,
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
// This is more than the maximum number of send_request calls than will fit into the block (execution time)
const ALEPH_MAX_REQUESTS_PER_BLOCK: usize = 50;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
enum RelayerError {
    #[error("An error which can be handled by restarting")]
    Recoverable(CircuitBreakerEvent),

    #[error("Ack receiver has dropper before the message could be delivered")]
    AckReceiverDropped,

    #[error("AlephZero node connection error")]
    AzeroConnection(#[from] connections::azero::Error),

    #[error("Ethereum node connection error")]
    EthereumConnection(#[from] connections::eth::EthConnectionError),

    #[error("Ethereum wallet error")]
    EthWallet(#[from] WalletError),

    #[error("Task join error")]
    Join(#[from] JoinError),

    #[error("AlephZero channel send error")]
    AzeroEventSend(#[from] mpsc::error::SendError<AzeroMostEvent>),

    #[error("Ethereum event channel send error")]
    EthEventSend(#[from] mpsc::error::SendError<EthMostEvent>),

    #[error("circuit breaker channel send error")]
    CircuitBreakerSend(#[from] mpsc::error::SendError<CircuitBreakerEvent>),

    #[error("ack receive error")]
    AckReceive(#[from] oneshot::error::RecvError),

    #[error("Advisory listener failure")]
    AdvisoryListener(#[from] AdvisoryListenerError),

    #[error("AlephZero Most listener failure")]
    AlephZeroListener(#[from] AzeroListenerError),

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
            "[AlephZero] Creating signed connection using a development key {}",
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
        info!("[Ethereum] Creating signed connection using a Signer client");
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
            "[Ethereum] Creating signed connection using a development key {}",
            &wallet.address()
        );
        eth::with_local_wallet(eth::connect(&config.eth_node_http_url).await, wallet).await?
    } else {
        panic!("Use dev mode or connect to a signer");
    };

    let eth_signed_connection = Arc::new(eth_signed_connection);

    let eth_connection = Arc::new(eth::connect(&config.eth_node_http_url).await);

    debug!("Established connection to the Ethereum node");

    // Create channels
    // TODO: tweak channel buffers
    let (eth_events_sender, eth_events_receiver) = mpsc::channel::<EthMostEvents>(1);
    let (eth_block_number_sender, eth_block_number_receiver1) = broadcast::channel(1);
    let eth_block_number_receiver2 = eth_block_number_sender.subscribe();
    let (azero_events_sender, azero_events_receiver) = mpsc::channel::<AzeroMostEvents>(1);
    let (azero_block_number_sender, azero_block_number_receiver1) = broadcast::channel(1);
    let azero_block_number_receiver2 = azero_block_number_sender.subscribe();
    let (circuit_breaker_sender, circuit_breaker_receiver) =
        mpsc::channel::<CircuitBreakerEvent>(1);
    let (eth_event_sender, eth_event_receiver) = mpsc::channel::<EthMostEvent>(1);
    let (azero_event_sender, azero_event_receiver) =
        mpsc::channel::<AzeroMostEvent>(ALEPH_MAX_REQUESTS_PER_BLOCK);

    let mut tasks = JoinSet::new();

    tasks.spawn(
        AdvisoryListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroHaltedListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            circuit_breaker_sender.clone(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumPausedListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            circuit_breaker_sender.clone(),
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumListener::run(
            Arc::clone(&config),
            Arc::clone(&eth_connection),
            eth_events_sender.clone(),
            eth_block_number_sender.clone(),
            eth_block_number_receiver1,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        EthereumEventsHandler::run(eth_events_receiver, eth_event_sender)
            .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroListener::run(
            Arc::clone(&config),
            Arc::clone(&azero_connection),
            azero_events_sender,
            azero_block_number_sender.clone(),
            azero_block_number_receiver1,
        )
        .map_err(RelayerError::from),
    );

    tasks.spawn(
        AlephZeroEventsHandler::run(azero_events_receiver, azero_event_sender)
            .map_err(RelayerError::from),
    );

    tasks.spawn(
        RedisManager::run(
            Arc::clone(&config),
            eth_block_number_sender.clone(),
            eth_block_number_receiver2,
            azero_block_number_sender.clone(),
            azero_block_number_receiver2,
        )
        .map_err(RelayerError::from),
    );

    // let circuit_breaker_receiver = Arc::new(Mutex::new(circuit_breaker_receiver));
    // let eth_events_receiver = Arc::new(Mutex::new(eth_events_receiver));
    // let azero_events_receiver = Arc::new(Mutex::new(azero_events_receiver));

    spawn_relayer(
        &mut tasks,
        config.clone(),
        circuit_breaker_receiver,
        eth_event_receiver,
        azero_event_receiver,
        azero_signed_connection.clone(),
        eth_signed_connection.clone(),
        circuit_breaker_sender.clone(),
    );

    let mut delay = Duration::from_secs(2);
    while let Some(result) = tasks.join_next().await {
        match result? {
            Ok(_) => error!("One of the core tasks has exited. This is fatal"),
            Err(RelayerError::Recoverable(from)) => {
                info!("Trying to recover from {from:?}");

                // spawn_relayer(
                //     &mut tasks,
                //     config.clone(),
                //     circuit_breaker_receiver,
                //     eth_event_receiver,
                //     azero_event_receiver,
                //     azero_signed_connection.clone(),
                //     eth_signed_connection.clone(),
                //     circuit_breaker_sender.clone(),
                // );

                sleep(max(Duration::from_secs(900), delay)).await;
                delay *= 2;
            }
            Err(why) => {
                error!("Fatal error in one of the core components: {why:?}");
                std::process::exit(1);
            }
        }
    }

    error!("We should have never gotten here!");
    std::process::exit(1);
}

#[allow(clippy::too_many_arguments)]
fn spawn_relayer(
    tasks: &mut JoinSet<Result<(), RelayerError>>,
    config: Arc<Config>,
    circuit_breaker_receiver: mpsc::Receiver<CircuitBreakerEvent>,
    // eth_events_receiver: Arc<Mutex<mpsc::Receiver<EthMostEvents>>>,
    // azero_events_receiver: Arc<Mutex<mpsc::Receiver<AzeroMostEvents>>>,
    eth_event_receiver: mpsc::Receiver<EthMostEvent>,
    azero_event_receiver: mpsc::Receiver<AzeroMostEvent>,

    azero_signed_connection: Arc<AzeroConnectionWithSigner>,
    eth_signed_connection: Arc<SignedEthConnection>,
    circuit_breaker_sender: mpsc::Sender<CircuitBreakerEvent>,
) {
    tasks.spawn(Relayer::run(
        config,
        circuit_breaker_receiver,
        eth_event_receiver,
        azero_event_receiver,
        azero_signed_connection,
        eth_signed_connection,
        circuit_breaker_sender,
    ));
}

pub struct Relayer;

impl Relayer {
    #[allow(clippy::too_many_arguments)]
    async fn run(
        config: Arc<Config>,
        mut circuit_breaker_receiver: mpsc::Receiver<CircuitBreakerEvent>,
        mut eth_event_receiver: mpsc::Receiver<EthMostEvent>,
        mut azero_event_receiver: mpsc::Receiver<AzeroMostEvent>,
        azero_signed_connection: Arc<AzeroConnectionWithSigner>,
        eth_signed_connection: Arc<SignedEthConnection>,
        circuit_breaker_sender: mpsc::Sender<CircuitBreakerEvent>,
    ) -> Result<(), RelayerError> {
        // let mut circuit_breaker_receiver = circuit_breaker_receiver.lock().await;
        // let mut eth_events_receiver = eth_events_receiver.lock().await;
        // let mut azero_events_receiver = azero_events_receiver.lock().await;

        loop {
            select! {
                Some (event) = circuit_breaker_receiver.recv () => {
                    warn!("Relayer is exiting due to a circuit breaker event {event:?}");
                    return Err(RelayerError::Recoverable (event));
                },

                Some (azero_event) = azero_event_receiver.recv () => {
                    let config = Arc::clone (&config);
                    let eth_connection = Arc::clone (&eth_signed_connection);

                    let circuit_breaker_sender_rc = Arc::new (circuit_breaker_sender.clone ()) ;

                    // Spawn a new task for handling each event
                    tokio::spawn (async move  {
                        let AzeroMostEvent { event, event_ack_sender } = azero_event;

                        let circuit_breaker_sender = Arc::clone (&circuit_breaker_sender_rc) ;

                        if let Err(why) = AlephZeroEventHandler::handle_event(config, eth_connection, event).await {
                            warn!("[AlephZero] event handler failed {why:?}");
                            circuit_breaker_sender.send (CircuitBreakerEvent::AlephZeroEventHandlerFailure).await.expect ("circuit breaker receiver has dropped before the message could be delivered");
                        }

                        info!("[AlephZero] Acknowledging event");
                        event_ack_sender.send (()).expect ("[AlephZero] event ack receiver has dropped before the message could be delivered");
                        info!("[AlephZero] Event acknowledged");
                    });
                },

                Some (eth_event) = eth_event_receiver.recv() => {
                    let EthMostEvent { event, event_ack_sender } = eth_event;
                    info!("[Ethereum] Received event {event:?}");

                    if let Err(why) = EthereumEventHandler::handle_event (event,  &config, &azero_signed_connection).await {
                        warn!("[Ethereum] event handler failure {why:?}");
                        circuit_breaker_sender.send (CircuitBreakerEvent::EthEventHandlerFailure).await? ;
                    }
                    info!("[Ethereum] Acknowledging event");
                    event_ack_sender.send(()).map_err(|_| RelayerError::AckReceiverDropped)?;
                    info!("[Ethereum] Event acknowledged");
                },

                else => {
                    debug!("Nothing to do, idling");
                }
            }
        }
    }
}
