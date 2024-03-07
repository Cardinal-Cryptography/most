use std::{
    process,
    sync::{atomic::AtomicBool, Arc},
};

use aleph_client::Connection;
use clap::Parser;
use crossbeam_channel::{
    bounded, select, unbounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender,
};
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, Client as RedisClient, RedisError};
use thiserror::Error;
use tokio::{
    sync::{mpsc, Mutex},
    task::JoinSet,
    time::{sleep, Duration},
};

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
    // Create channels
    let (eth_sender, eth_receiver) = unbounded::<String>();
    let (azero_sender, azero_receiver) = unbounded::<String>();
    let (circuit_breaker_sender, circuit_breaker_receiver) = unbounded::<CircuitBreakerEvent>();

    // Spawn tasks for listening to channels
    let task1 = tokio::spawn(listen_channel(
        "EthEventHandler",
        eth_receiver,
        circuit_breaker_receiver.clone(),
        circuit_breaker_sender.clone(),
        test_handler,
    ));
    let task2 = tokio::spawn(listen_channel(
        "AzeroEventHandler",
        azero_receiver,
        circuit_breaker_receiver.clone(),
        circuit_breaker_sender.clone(),
        test_handler,
    ));

    tokio::try_join!(task1, task2).unwrap();
    std::process::exit(1);
}

async fn listen_channel<F>(
    name: &'static str,
    event_receiver: CrossbeamReceiver<String>,
    circuit_breaker_receiver: CrossbeamReceiver<CircuitBreakerEvent>,
    circuit_breaker_sender: CrossbeamSender<CircuitBreakerEvent>,
    handle_event: F,
) where
    F: Fn(String) -> bool + Send + 'static,
{
    loop {
        select! {
            recv(event_receiver) -> event => match event {
                Ok(evt) => match handle_event (evt) {
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
                    CircuitBreakerEvent::EventHandlerSuccess => todo!(),
                    CircuitBreakerEvent::EventHandlerFailure => todo!(),
                    CircuitBreakerEvent::BridgeHaltAzero => todo!(),
                    CircuitBreakerEvent::BridgeHaltEth => todo!(),
                    CircuitBreakerEvent::AdvisoryEmergency => todo!(),
                    CircuitBreakerEvent::Other (why) => todo!(),
                },
                Err(why) => {
                    circuit_breaker_sender.send (CircuitBreakerEvent::Other (format! ("{why}"))).expect ("{name} can send to the circuit breaker channel")
                },
            }
        }
    }
}

fn test_handler(msg: String) -> bool {
    true
}
