use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use aleph_client::utility::BlocksApi;
use futures::future::join_all;
use log::{info, warn};
use thiserror::Error;
use tokio::{
    sync::{
        broadcast::{self, error::SendError},
        mpsc,
    },
    time::sleep,
};

use super::ALEPH_BLOCK_PROD_TIME_SEC;
use crate::{
    config::Config,
    connections::azero::AzeroWsConnection,
    contracts::{AdvisoryInstance, AzeroContractError},
    CircuitBreakerEvent,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AdvisoryListenerError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    // #[error("broadcast send error")]
    // Broadcast(#[from] broadcast::error::SendError<CircuitBreakerEvent>),
    #[error("channel send error")]
    Send(#[from] mpsc::error::SendError<CircuitBreakerEvent>),
}

pub struct AdvisoryListener;

impl AdvisoryListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        circuit_breaker_sender: mpsc::Sender<CircuitBreakerEvent>,
    ) -> Result<(), AdvisoryListenerError> {
        let Config {
            advisory_contract_metadata,
            advisory_contract_addresses,
            ..
        } = &*config;

        let contracts: Vec<AdvisoryInstance> = advisory_contract_addresses
            .clone()
            .expect("Advisory addresses")
            .into_iter()
            .try_fold(
                Vec::new(),
                |mut acc, address| -> Result<Vec<AdvisoryInstance>, AdvisoryListenerError> {
                    acc.push(AdvisoryInstance::new(&address, advisory_contract_metadata)?);
                    Ok(acc)
                },
            )?;

        loop {
            let all: Vec<_> = contracts
                .iter()
                .map(|advisory| advisory.is_emergency(&azero_connection))
                .collect();

            for maybe_emergency in join_all(all).await {
                match maybe_emergency {
                    Ok((is_emergency, address)) => {
                        if is_emergency {
                            circuit_breaker_sender
                                .send(CircuitBreakerEvent::AdvisoryEmergency(address))
                                .await?;
                            break;
                        }
                    }
                    Err(why) => return Err(AdvisoryListenerError::AzeroContract(why)),
                }
            }

            // sleep for about half a block production time before making another round of queries
            sleep(Duration::from_millis(
                (ALEPH_BLOCK_PROD_TIME_SEC * 1000) / 2,
            ))
            .await;
        }
    }
}
