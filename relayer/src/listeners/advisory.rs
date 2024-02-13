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
use tokio::time::sleep;

use crate::{
    config::Config,
    connections::azero::AzeroWsConnection,
    contracts::{AdvisoryInstance, AzeroContractError},
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AdvisoryListenerError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),
}

pub struct AdvisoryListener;

impl AdvisoryListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        emergency: Arc<AtomicBool>,
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
            let previous_emergency_state = emergency.load(Ordering::Relaxed);
            let mut current_emergency_state = false;

            let all: Vec<_> = contracts
                .iter()
                .map(|advisory| advisory.is_emergency(&azero_connection))
                .collect();

            for maybe_emergency in join_all(all).await {
                match maybe_emergency {
                    Ok((is_emergency, address)) => {
                        if is_emergency {
                            current_emergency_state = true;
                            if current_emergency_state != previous_emergency_state {
                                let current_block_number =
                                    azero_connection.get_block_number_opt(None).await?;
                                warn!("Detected an emergency state at block {current_block_number:?} in an Advisory contract {address}");
                            }
                            break;
                        }
                    }
                    Err(why) => return Err(AdvisoryListenerError::AzeroContract(why)),
                }
            }

            if previous_emergency_state && !current_emergency_state {
                info!("Previously set emergency state has been lifted");
            }

            emergency.store(current_emergency_state, Ordering::Relaxed);

            // we sleep for about half a block production time before making another round of queries
            sleep(Duration::from_millis(500)).await;
        }
    }
}
