use std::{
    process,
    sync::{atomic::AtomicBool, Arc},
};

use aleph_client::Connection;
use clap::Parser;
use config::Config;
use connections::azero::AzeroConnectionWithSigner;
// use crossbeam_channel::{
//     bounded, select, unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender,
// };
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use futures::Future;
use handlers::{handle_events as handle_eth_events, EthHandlerError};
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
    listeners::{EthListener, EthMostEvents},
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
    EventHandlerSuccess,
    EventHandlerFailure,
    BridgeHaltAzero,
    BridgeHaltEth,
    AdvisoryEmergency,
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
    let (eth_block_number_sender, eth_block_number_receiver1) = broadcast::channel(1);
    let mut eth_block_number_receiver2 = eth_block_number_sender.subscribe();
    let (circuit_breaker_sender, circuit_breaker_receiver) =
        broadcast::channel::<CircuitBreakerEvent>(1);

    // TODO : advisory listener task
    // TODO : halted listener tasks
    // TODO : azero event handling tasks (publisher and consumer)

    // let process_message =
    //     |events: Message, config: Arc<Config>, azero_connection: Arc<AzeroConnectionWithSigner>| {
    //         tokio::spawn(async move { handle_eth_events(events, &config, &azero_connection).await })
    //     };

    let eth_listener = tokio::spawn(EthListener::run(
        Arc::clone(&config),
        eth_connection,
        eth_events_sender,
        eth_block_number_sender.clone(),
        eth_block_number_receiver1,
    ));

    let redis_manager = tokio::spawn(RedisManager::run(
        Arc::clone(&config),
        eth_block_number_sender.clone(),
        eth_block_number_receiver2,
    ));

    let eth_handler = tokio::spawn(EthHandler::run(
        eth_events_receiver,
        circuit_breaker_receiver,
        // circuit_breaker_sender.clone(),
        Arc::clone(&config),
        Arc::clone(&azero_signed_connection),
    ));

    // tokio::try_join!(task1, task2).expect("Listener task should never finish");
    // TODO: handle restart, or crash and rely on k8s
    std::process::exit(1);
}

// pub struct RequestHandler;

// impl RequestHandler {
//     pub async fn run(config: Arc<Config>) {
//         loop {
//             select! {

//                 todo!("")

//             }
//         }
//     }
// }

// TODO: select between all event channels
// async fn handle_requests<F>(
//     mut eth_event_receiver: mpsc::Receiver<Message>,
//     mut circuit_breaker_receiver: mpsc::Receiver<CircuitBreakerEvent>,
//     circuit_breaker_sender: mpsc::Sender<CircuitBreakerEvent>,
//     config: Arc<Config>,
//     azero_connection: Arc<AzeroConnectionWithSigner>,
//     process_eth_message: F,
// ) where
//     F: Fn(
//             Message,
//             Arc<Config>,
//             Arc<AzeroConnectionWithSigner>,
//         ) -> JoinHandle<Result<(), EthHandlerError>>
//         + Send,
// {
//     loop {
//         tokio::select! {
//             Some(eth_events) = eth_event_receiver.recv() => {
//                 if let Ok(CircuitBreakerEvent::EventHandlerFailure) = circuit_breaker_receiver.try_recv() {
//                     // println!("{} Circuit breaker fired. Dropping task and restarting.", name);
//                     return; // Drop the task and restart
//                 }

//                 let processing_result = process_eth_message(eth_events, Arc::clone (&config), Arc::clone (&azero_connection)).await;
//                 // if processing_result {
//                     circuit_breaker_sender.send(CircuitBreakerEvent::EventHandlerSuccess).await.unwrap();
//                 // } else {
//                 //     circuit_breaker_tx.send(CircuitBreakerEvent::Failure).await.unwrap();
//                 // }
//             }
//         }
//     }
// }
