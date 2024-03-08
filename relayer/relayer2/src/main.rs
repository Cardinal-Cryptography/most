use std::{
    process,
    sync::{atomic::AtomicBool, Arc},
};

use aleph_client::Connection;
use clap::Parser;
use config::Config;
use connections::azero::AzeroConnectionWithSigner;
use crossbeam_channel::{
    bounded, select, unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender,
};
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use handlers::handle_event as handle_eth_event;
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinSet,
    time::{sleep, Duration},
};

use crate::{connections::azero, contracts::MostEvents};

mod config;
mod connections;
mod contracts;
mod handlers;
mod helpers;
// mod listeners;

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
async fn main() {
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

    debug!("Established connection to Aleph Zero node");

    // Create channels
    let (eth_sender, eth_receiver) = unbounded::<MostEvents>();
    // let (azero_sender, azero_receiver) = unbounded::<String>();
    let (circuit_breaker_sender, circuit_breaker_receiver) = unbounded::<CircuitBreakerEvent>();

    // Spawn tasks for listening to channels

    let task1 = tokio::spawn(listen_eth_channel(
        "EthEventHandler",
        eth_receiver,
        circuit_breaker_receiver.clone(),
        circuit_breaker_sender.clone(),
        &config,
        &azero_signed_connection,
    ));

    tokio::try_join!(task1).unwrap();
    std::process::exit(1);
}

async fn listen_eth_channel<F>(
    name: &'static str,
    event_receiver: CrossbeamReceiver<MostEvents>,
    circuit_breaker_receiver: CrossbeamReceiver<CircuitBreakerEvent>,
    circuit_breaker_sender: CrossbeamSender<CircuitBreakerEvent>,
    config: &Config,
    azero_connection: &AzeroConnectionWithSigner,
    // handle_event: F,
)
// where
//     F: Fn(MostEvents, Config) -> bool + Send + 'static,
{
    loop {
        select! {
            recv(event_receiver) -> event => match event {
                Ok(evt) => match handle_eth_event (&evt, config, azero_connection) {
                    true => circuit_breaker_sender.send (CircuitBreakerEvent::EventHandlerSuccess).expect ("{name} can send to the circuit breaker channel"),
                    false => circuit_breaker_sender.send (CircuitBreakerEvent::EventHandlerFailure).expect ("{name} can send to the circuit breaker channel")
                },
                Err(why) => {
                    error!("{name} fatal error: {why}");
                    std::process::exit(1);
                }
            },

            recv(circuit_breaker_receiver) -> msg => match msg {
                Ok(circuit_breaker_event) => match circuit_breaker_event {
                    CircuitBreakerEvent::EventHandlerSuccess => todo!(), // nothing to do?
                    CircuitBreakerEvent::EventHandlerFailure => todo!(), // try 3 times and give up?
                    CircuitBreakerEvent::BridgeHaltAzero => todo!(), // go into a query loop
                    CircuitBreakerEvent::BridgeHaltEth => todo!(), // go into a query loop
                    CircuitBreakerEvent::AdvisoryEmergency => todo!(), // go into a query loop
                    CircuitBreakerEvent::Other (why) => todo!(), // try 3 times and give up?
                },
                Err(why) => {
                    circuit_breaker_sender.send (CircuitBreakerEvent::Other (format! ("{why}"))).expect ("{name} can send to the circuit breaker channel")
                },
            }
        }
    }
}

fn test_handler(
    event: &MostEvents,
    config: &Config,
    azero_connection: &AzeroConnectionWithSigner,
) -> bool {
    true
}
