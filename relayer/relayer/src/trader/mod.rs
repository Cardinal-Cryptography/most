use std::{cmp::min, sync::Arc, time::Duration};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    pallets::system::SystemApi,
    utility::BlocksApi,
    AlephConfig, AsConnection, Connection, SignedConnectionApi,
};
use futures::stream::{FuturesOrdered, StreamExt};
use log::{debug, error, info, warn};
use subxt::{events::Events, storage::address};
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
    contracts::{AzeroContractError, MostInstance, WrappedAzeroInstance},
    CircuitBreakerEvent,
};

// trader component will sell the surplus
pub const AZERO_SURPLUS_LIMIT: u128 = 1_000_000_000_000; // 1 AZERO

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

    #[error("missing required arg {0}")]
    MissingRequired(String),
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
            azero_wrapped_azero_address,
            azero_wrapped_azero_metadata,
            ..
        } = &*config;
        // let mut tasks = JoinSet::new();

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        let address = &azero_wrapped_azero_address
            .clone()
            .ok_or(TraderError::MissingRequired(
                "azero_wrapped_azero_address".to_owned(),
            ))?;

        let wrapped_azero = WrappedAzeroInstance::new(
            address,
            azero_wrapped_azero_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        info!("Starting");

        loop {
            debug!("Ping");

            // TODO wrap Azero
            let whoami = azero_connection.account_id();

            let balance = azero_connection
                .get_free_balance(whoami.to_owned(), None)
                .await;

            if balance > AZERO_SURPLUS_LIMIT {
                let surplus = balance.saturating_sub(AZERO_SURPLUS_LIMIT);
                info!("{whoami} has {surplus} A0 above the set limit of {AZERO_SURPLUS_LIMIT} A0 that will be swapped");

                // _ =
            }

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
