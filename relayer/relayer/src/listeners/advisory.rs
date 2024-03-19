use std::{sync::Arc, time::Duration};

use futures::future::join_all;
use log::{debug, warn};
use thiserror::Error;
use tokio::{select, sync::broadcast, time::sleep};

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
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("broadcast send error")]
    BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),

    #[error("broadcast receive error")]
    BroadcastReceive(#[from] broadcast::error::RecvError),
}

pub struct AdvisoryListener;

impl AdvisoryListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AdvisoryListenerError> {
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
            select! {
                cb_event = circuit_breaker_receiver.recv () => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                else => {
                    let all: Vec<_> = contracts
                        .iter()
                        .map(|advisory| advisory.is_emergency(&azero_connection))
                        .collect();

                    for maybe_emergency in join_all(all).await {
                        match maybe_emergency {
                            Ok((is_emergency, address)) => {
                                if is_emergency {
                                    debug!("Emergency state in one of the advisory contracts {address}");
                                    let status = CircuitBreakerEvent::AdvisoryEmergency(address);
                                    circuit_breaker_sender.send(status.clone())?;
                                    return Ok(status.clone());
                                }
                            }
                            Err(why) => return Err(AdvisoryListenerError::AzeroContract(why)),
                        }
                    }

                    // sleep for a block production time before making another round of queries
                    sleep(Duration::from_secs(ALEPH_BLOCK_PROD_TIME_SEC)).await;
                }
            }
        }
    }
}
