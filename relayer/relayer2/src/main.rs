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
    Success,
    Failure,
    TestRequest(bool), // bool indicates success or failure of the test request
    Timeout,
}

#[tokio::main]
async fn main() {
    // Create channels
    let (eth_sender, eth_receiver) = bounded::<String>(10);
    let (azero_sender, azero_receiver) = bounded::<String>(10);
    let (circuit_breaker_sender, circuit_breaker_receiver) = bounded::<CircuitBreakerEvent>(1); //mpsc::channel::<CircuitBreakerEvent>(1);

    // Spawn tasks for listening to channels
    let task1 = tokio::spawn(listen_channel(
        "EthEventHandler",
        eth_receiver,
        circuit_breaker_receiver.clone(),
        circuit_breaker_sender.clone(),
    ));
    let task2 = tokio::spawn(listen_channel(
        "AzeroEventHandler",
        azero_receiver,
        circuit_breaker_receiver.clone(),
        circuit_breaker_sender.clone(),
    ));

    // Wait for tasks to complete
    tokio::try_join!(task1, task2).unwrap();
}

async fn listen_channel(
    name: &'static str,
    mut event_receiver: CrossbeamReceiver<String>,
    circuit_breaker_receiver: CrossbeamReceiver<CircuitBreakerEvent>,
    circuit_breaker_sender: CrossbeamSender<CircuitBreakerEvent>,
) {
    let mut consecutive_failures = 0;
    let mut restart_attempts = 0;
    let max_restart_attempts = 3;

    loop {
        select! {
            recv(event_receiver) -> msg => println!("TODO"),
            recv(circuit_breaker_receiver) -> msg => match msg {
                Ok(circuit_breaker_event) => match circuit_breaker_event {
                    CircuitBreakerEvent::Success => todo!(),
                    CircuitBreakerEvent::Failure => todo!(),
                    CircuitBreakerEvent::TestRequest(_) => todo!(),
                    CircuitBreakerEvent::Timeout => todo!(),
                },
                Err(why) => {
                    error!("{name} fatal error: {why}");
                    std::process::exit(1);
                },
            }
        }
    }
}
