use std::{
    process,
    sync::{atomic::AtomicBool, mpsc::Receiver, Arc},
};

use aleph_client::{AccountId, Connection};
use clap::Parser;
use config::Config;
use connections::azero::AzeroConnectionWithSigner;
// use crossbeam_channel::{
//     bounded, select, unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender,
// };
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use futures::Future;
use handlers::EthHandlerError;
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot, Mutex},
    task::{self, JoinHandle, JoinSet},
    time::{sleep, Duration},
};

use crate::{
    connections::{azero, eth},
    contracts::MostEvents,
    handlers::EthHandler,
    listeners::{
        AdvisoryListener, AlephZeroListener, AzeroMostEvents, EthListener, EthMostEvent,
        EthMostEvents,
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

#[derive(Debug, Clone)]
enum CircuitBreakerEvent {
    // EventHandlerSuccess,
    EthEventHandlerFailure,
    BridgeHaltAzero,
    BridgeHaltEth,
    AdvisoryEmergency(AccountId),
    Other(String),
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

    debug!("Established connection to Ethereum node");

    // Create channels
    let (eth_events_sender, eth_events_receiver) = mpsc::channel::<EthMostEvents>(1);
    let (eth_event_sender, eth_event_receiver) = mpsc::channel::<EthMostEvent>(1);
    let (eth_block_number_sender, eth_block_number_receiver1) = broadcast::channel(1);
    let mut eth_block_number_receiver2 = eth_block_number_sender.subscribe();

    let (azero_events_sender, azero_events_receiver) = mpsc::channel::<AzeroMostEvents>(1);

    let (azero_block_number_sender, azero_block_number_receiver1) = broadcast::channel(1);
    let mut azero_block_number_receiver2 = azero_block_number_sender.subscribe();

    let (circuit_breaker_sender, circuit_breaker_receiver) =
        broadcast::channel::<CircuitBreakerEvent>(1);

    // TODO : halted listener tasks
    // TODO : circuit breaker
    // TODO : azero event handling tasks (publisher and consumer)

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
        azero_block_number_sender,
        azero_block_number_receiver1,
    ));

    let redis_manager = tokio::spawn(RedisManager::run(
        Arc::clone(&config),
        eth_block_number_sender.clone(),
        eth_block_number_receiver2,
    ));

    // tokio::try_join!(task1, task2).expect("Listener task should never finish");
    // TODO: handle restarts
    std::process::exit(1);
}

pub struct Relayer;

impl Relayer {
    pub async fn run(
        config: Arc<Config>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
        mut eth_events_receiver: mpsc::Receiver<EthMostEvents>,
        mut eth_event_receiver: mpsc::Receiver<EthMostEvent>,
        eth_event_sender: mpsc::Sender<EthMostEvent>,
        azero_connection: Arc<AzeroConnectionWithSigner>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
    ) {
        loop {
            select! {
                circuit_breaker_event = circuit_breaker_receiver.recv () => {
                    // TODO: close circuit
                    // todo!("")
                    // return;
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

                    info!("Acknowledging Eth events batch receipt");
                    events_ack_sender.send(()).unwrap ();
                },

                Some (eth_event) = eth_event_receiver.recv() => {
                    let EthMostEvent { event, event_ack_sender } = eth_event;
                    info!("Received an Eth event {event:?}");

                    if let Err(why) = EthHandler::handle_event (event,  &config, &azero_connection).await {
                        warn!("Eth event handler failure {why:?}");
                        circuit_breaker_sender.send (CircuitBreakerEvent::EthEventHandlerFailure).unwrap ();
                    }
                    info!("Acknowledging Eth event receipt");
                    event_ack_sender.send(()).unwrap ();
                }

                else => break
            }
        }
    }
}
