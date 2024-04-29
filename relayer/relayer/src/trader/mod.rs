use std::{cmp::min, sync::Arc, time::Duration};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    pallets::system::SystemApi,
    utility::BlocksApi,
    AlephConfig, AsConnection, Connection, SignedConnectionApi,
};
use futures::stream::{FuturesOrdered, StreamExt};
use log::{debug, error, info, warn};
use subxt::events::Events;
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    task::{JoinError, JoinSet},
    time::sleep,
};

use super::AzeroMostEvents;
use crate::{
    config::Config,
    connections::azero::{AzeroConnectionWithSigner, AzeroWsConnection},
    contracts::{AzeroContractError, MostInstance},
    CircuitBreakerEvent,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum TraderError {
    #[error("Azero contract error {0}")]
    AzeroContract(#[from] AzeroContractError),

    // #[error("broadcast send error")]
    // BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),
    #[error("broadcast receive error {0}")]
    BroadcastReceive(#[from] broadcast::error::RecvError),
}

#[derive(Copy, Clone)]
pub struct Trader;

impl Trader {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: &AzeroConnectionWithSigner,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, TraderError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            azero_ref_time_limit,
            azero_proof_size_limit,
            ..
        } = &*config;
        // let mut tasks = JoinSet::new();

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        info!("Starting");

        loop {
            debug!("Ping");

            // TODO wrap Azero
            let whoami = azero_connection.account_id();


            // azero_connection.get_free_balance(account, at)

            // TODO swap wAzero to wETH
            // TODO bridge wETH to ETHEREUM
            // TODO unwrap 0xWETH -> ETH

            // select! {
            //     cb_event = circuit_breaker_receiver.recv () => {
            //         warn!("Exiting due to a circuit breaker event {cb_event:?}");
            //         return Ok(cb_event?);
            //     },

            // }
        }
    }
}
