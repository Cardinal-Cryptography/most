use std::{cmp::min, sync::Arc, time::Duration};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    utility::BlocksApi,
    AlephConfig, AsConnection, Connection,
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
    // #[error("Azero contract error")]
    // AzeroContract(#[from] AzeroContractError),

    // #[error("broadcast send error")]
    // BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    // #[error("broadcast receive error")]
    // BroadcastReceive(#[from] broadcast::error::RecvError),
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
        //

        //
        todo!("")
    }
}
