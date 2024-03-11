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
use handlers::{handle_event as handle_eth_event, EthHandlerError};
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{
    sync::{mpsc, Mutex},
    task,
    task::{JoinHandle, JoinSet},
    time::{sleep, Duration},
};

use crate::{
    connections::{azero, eth},
    contracts::MostEvents,
    listeners::EthListener,
};

mod config;
mod connections;
mod contracts;
mod handlers;
mod helpers;
mod listeners;

const DEV_MNEMONIC: &str =
    "harsh master island dirt equip search awesome double turn crush wool grant";

#[derive(Debug)]
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
    let (eth_sender, eth_receiver) = mpsc::channel::<MostEvents>(1);
    let (circuit_breaker_sender, circuit_breaker_receiver) =
        mpsc::channel::<CircuitBreakerEvent>(1);

    // TODO : advisory listener task
    // TODO : halted listener tasks
    // TODO : azero event handling tasks (publisher and consumer)

    let process_message =
        |event: MostEvents,
         config: Arc<Config>,
         azero_connection: Arc<AzeroConnectionWithSigner>| {
            tokio::spawn(async move { handle_eth_event(&event, &config, &azero_connection).await })
        };

    let task1 = tokio::spawn(listen_channel(
        eth_receiver,
        circuit_breaker_receiver,
        circuit_breaker_sender.clone(),
        Arc::clone(&config),
        Arc::clone(&azero_signed_connection),
        process_message,
    ));

    let task2 = tokio::spawn(EthListener::run(
        config,
        // Arc::clone(&azero_signed_connection),
        eth_connection,
        redis_connection,
        eth_sender,
    ));

    tokio::try_join!(task1, task2).expect("Listener task should never finish");
    // TODO: handle restart, or crash and rely on k8s
    std::process::exit(1);
}

// TODO: select between all event channels
async fn listen_channel<F>(
    mut event_receiver: mpsc::Receiver<MostEvents>,
    mut circuit_breaker_receiver: mpsc::Receiver<CircuitBreakerEvent>,
    circuit_breaker_sender: mpsc::Sender<CircuitBreakerEvent>,
    config: Arc<Config>,
    azero_connection: Arc<AzeroConnectionWithSigner>,
    process_message: F,
) where
    F: Fn(
            MostEvents,
            Arc<Config>,
            Arc<AzeroConnectionWithSigner>,
        ) -> JoinHandle<Result<(), EthHandlerError>>
        + Send,
{
    loop {
        tokio::select! {
            Some(event) = event_receiver.recv() => {
                if let Ok(CircuitBreakerEvent::EventHandlerFailure) = circuit_breaker_receiver.try_recv() {
                    // println!("{} Circuit breaker fired. Dropping task and restarting.", name);
                    return; // Drop the task and restart
                }

                // println!("{} received message: {}", name, msg);
                // Call the custom processing function and wait for its completion
                let processing_result = process_message(event, Arc::clone (&config), Arc::clone (&azero_connection)).await;
                // if processing_result {
                    circuit_breaker_sender.send(CircuitBreakerEvent::EventHandlerSuccess).await.unwrap();
                // } else {
                //     circuit_breaker_tx.send(CircuitBreakerEvent::Failure).await.unwrap();
                // }
            }
        }
    }
}
