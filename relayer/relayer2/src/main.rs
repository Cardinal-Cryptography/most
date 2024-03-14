use std::sync::{atomic::AtomicBool, Arc};

use aleph_client::AccountId;
use clap::Parser;
use config::Config;
use connections::{azero::AzeroConnectionWithSigner, eth::SignedEthConnection};
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer};
use eyre::Result;
use log::{debug, info, warn};
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    task::JoinSet,
};

use crate::{
    connections::{azero, eth},
    handlers::{AlephZeroHandler, EthHandler},
    listeners::{
        AdvisoryListener, AlephZeroListener, AzeroMostEvent, AzeroMostEvents, EthListener,
        EthMostEvent, EthMostEvents,
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

#[derive(Debug, Clone)]
enum CircuitBreakerEvent {
    EthEventHandlerFailure,
    AlephZeroEventHandlerFailure,
    BridgeHaltAzero,
    BridgeHaltEth,
    AdvisoryEmergency(AccountId),
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::parse());
    env_logger::init();

    info!("{:#?}", &config);

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
    let azero_signed_connection = Arc::new(azero_signed_connection);

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

    debug!("Established connection to the Ethereum node");

    // Create channels
    let (eth_events_sender, eth_events_receiver) = mpsc::channel::<EthMostEvents>(1);
    let (eth_event_sender, eth_event_receiver) = mpsc::channel::<EthMostEvent>(1);
    let (eth_block_number_sender, eth_block_number_receiver1) = broadcast::channel(1);
    let mut eth_block_number_receiver2 = eth_block_number_sender.subscribe();
    let (azero_events_sender, azero_events_receiver) = mpsc::channel::<AzeroMostEvents>(1);
    let (azero_event_sender, azero_event_receiver) =
        mpsc::channel::<AzeroMostEvents>(ALEPH_MAX_REQUESTS_PER_BLOCK);
    let (azero_block_number_sender, azero_block_number_receiver1) = broadcast::channel(1);
    let mut azero_block_number_receiver2 = azero_block_number_sender.subscribe();
    let (circuit_breaker_sender, circuit_breaker_receiver) =
        mpsc::channel::<CircuitBreakerEvent>(1);

    // TODO : halted listener tasks
    // TODO : publish & handle circuit breaker events

    let is_circuit_open = Arc::new(AtomicBool::new(true));

    let advisory = tokio::spawn(AdvisoryListener::run(
        Arc::clone(&config),
        Arc::clone(&azero_connection),
        circuit_breaker_sender.clone(),
    ));

    let eth_listener = tokio::spawn(EthListener::run(
        Arc::clone(&config),
        eth_connection,
        eth_events_sender,
        eth_block_number_sender.clone(),
        eth_block_number_receiver1,
    ));

    let azero_listener = tokio::spawn(AlephZeroListener::run(
        Arc::clone(&config),
        Arc::clone(&azero_connection),
        azero_events_sender,
        azero_block_number_sender.clone(),
        azero_block_number_receiver1,
    ));

    let redis_manager = tokio::spawn(RedisManager::run(
        Arc::clone(&config),
        eth_block_number_sender.clone(),
        eth_block_number_receiver2,
        azero_block_number_sender.clone(),
        azero_block_number_receiver2,
    ));

    // tokio::try_join!(task1, task2).expect("Listener task should never finish");
    // TODO: handle restarts
    std::process::exit(1);
}

pub struct Relayer;

impl Relayer {
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        config: Arc<Config>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
        mut eth_events_receiver: mpsc::Receiver<EthMostEvents>,
        mut eth_event_receiver: mpsc::Receiver<EthMostEvent>,
        eth_event_sender: mpsc::Sender<EthMostEvent>,
        mut azero_events_receiver: mpsc::Receiver<AzeroMostEvents>,
        azero_event_sender: mpsc::Sender<AzeroMostEvent>,
        mut azero_event_receiver: mpsc::Receiver<AzeroMostEvent>,
        azero_connection: Arc<AzeroConnectionWithSigner>,
        eth_signed_connection: Arc<SignedEthConnection>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
    ) {
        loop {
            select! {

                circuit_breaker_event = circuit_breaker_receiver.recv () => {
                    // TODO: close circuit
                    // todo!("")
                    // return;
                },

                Some (azero_events) = azero_events_receiver.recv () => {
                    let AzeroMostEvents { events, events_ack_sender } = azero_events;
                    info!("Received a batch of {} Azero events", events.len ());

                    // let mut acks = Vec::new ();
                    let mut acks = JoinSet::new();

                    for event in events {
                        let (event_ack_sender, event_ack_receiver) = oneshot::channel::<()>();
                        info!("Sending AlephZero event {event:?}");
                        azero_event_sender.send(AzeroMostEvent {event, event_ack_sender}).await.unwrap ();
                        acks.spawn(event_ack_receiver);
                    }

                    // wait for all concurrent tasks to finish
                    info!("Awaiting for events to be handled");
                    while let Some(_res) = acks.join_next().await {
                        // TODO: add why and send to circuit breaker channel
                    }

                    info!("Acknowledging Azero events batch as handled");
                    // marks the batch as done and releases the listener
                    events_ack_sender.send(()).unwrap ();
                },

                Some (azero_event) = azero_event_receiver.recv () => {
                    let config = Arc::clone (&config);
                    let eth_connection = Arc::clone (&eth_signed_connection);

                    let circuit_breaker_sender_rc = Arc::new (circuit_breaker_sender.clone ()) ;

                    // Spawn a new task for handling each event
                    tokio::spawn (async move  {
                        let AzeroMostEvent { event, event_ack_sender } = azero_event;

                        let circuit_breaker_sender = Arc::clone (&circuit_breaker_sender_rc) ;

                        if let Err(why) = AlephZeroHandler::handle_event(config, eth_connection, event).await {
                            warn!("AlephZero event handler failed {why:?}");
                            circuit_breaker_sender.send (CircuitBreakerEvent::AlephZeroEventHandlerFailure).unwrap ();
                        }

                        info!("Acknowledging AlephZero event as handled");
                        event_ack_sender.send (()).unwrap ();
                    });
                },

                Some(eth_events) = eth_events_receiver.recv() => {
                    let EthMostEvents { events, events_ack_sender } = eth_events;
                    info!("Received a batch {} of Eth events", events.len ());

                    for event in events {
                        let (event_ack_sender, event_ack_receiver) = oneshot::channel::<()>();
                        info!("Sending Eth event {event:?}");
                        eth_event_sender.send(EthMostEvent {event, event_ack_sender}).await.unwrap ();
                        info!("Awaiting event ack");
                        event_ack_receiver.await.unwrap ();
                        info!("Event ack received");
                    }

                    info!("Acknowledging Eth events batch as handled");
                    // marks the batch as done and releases the listener
                    events_ack_sender.send(()).unwrap ();
                },

                Some (eth_event) = eth_event_receiver.recv() => {
                    let EthMostEvent { event, event_ack_sender } = eth_event;
                    info!("Received an Eth event {event:?}");

                    if let Err(why) = EthHandler::handle_event (event,  &config, &azero_connection).await {
                        warn!("Eth event handler failure {why:?}");
                        circuit_breaker_sender.send (CircuitBreakerEvent::EthEventHandlerFailure).unwrap ();
                    }
                    info!("Acknowledging Eth event as handled");
                    event_ack_sender.send(()).unwrap ();
                }

                else => {}
            }
        }
    }
}
