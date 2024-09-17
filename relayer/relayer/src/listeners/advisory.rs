use std::{sync::Arc, time::Duration};

use contracts_azero_client::AccountId;
use futures::future::join_all;
use log::{debug, info, warn};
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
        advisories: Arc<Vec<AdvisoryInstance>>,
        azero_connection: Arc<AzeroWsConnection>,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, AdvisoryListenerError> {
        loop {
            debug!("Ping");

            select! {
                cb_event = circuit_breaker_receiver.recv() => {
                    warn!("Exiting due to a circuit breaker event {cb_event:?}");
                    return Ok(cb_event?);
                },

                active_advisories_res = Self::query_active_advisories(
                    advisories.clone(),
                    azero_connection.clone(),
                ) => {
                    debug!("Querying");

                    match active_advisories_res {
                        Err(why) => {
                            warn!("Exiting due to an error querying active advisories {why:?}");
                            let status = CircuitBreakerEvent::AlephClientError;
                            circuit_breaker_sender.send(status.clone())?;
                            return Ok(status.clone());
                        },
                        Ok(advisories) => {
                            if advisories.is_empty() {
                                debug!("No active advisories");
                            } else {
                                warn!("Exiting due to activation of advisories {advisories:?}");
                                let status = CircuitBreakerEvent::AdvisoryEmergency(advisories);
                                circuit_breaker_sender.send(status.clone())?;
                                return Ok(status.clone());
                            }
                        },
                    }
                }
            }

            sleep(Duration::from_secs(ALEPH_BLOCK_PROD_TIME_SEC)).await;
        }
    }

    pub async fn query_active_advisories(
        advisories: Arc<Vec<AdvisoryInstance>>,
        azero_connection: Arc<AzeroWsConnection>,
    ) -> Result<Vec<AccountId>, AdvisoryListenerError> {
        join_all(
            advisories
                .iter()
                .map(|advisory| advisory.is_emergency(&azero_connection))
                .collect::<Vec<_>>(),
        )
        .await
        .into_iter()
        .filter_map(|maybe_emergency| match maybe_emergency {
            Ok((true, address)) => Some(Ok(address)),
            Ok((false, _)) => None,
            Err(why) => Some(Err(AdvisoryListenerError::AzeroContract(why))),
        })
        .collect()
    }

    pub fn parse_advisory_addresses(config: Arc<Config>) -> Vec<AdvisoryInstance> {
        let Config {
            advisory_contract_metadata,
            advisory_contract_addresses,
            ..
        } = &*config;

        info!("Starting");

        advisory_contract_addresses
            .clone()
            .expect("Advisory addresses")
            .into_iter()
            .try_fold(
                Vec::new(),
                |mut acc, address| -> Result<Vec<AdvisoryInstance>, AdvisoryListenerError> {
                    acc.push(AdvisoryInstance::new(&address, advisory_contract_metadata)?);
                    Ok(acc)
                },
            )
            .expect("Advisory addresses list")
    }
}
