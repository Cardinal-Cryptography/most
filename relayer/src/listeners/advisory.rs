use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use aleph_client::utility::BlocksApi;
use log::{info, trace, warn};
use thiserror::Error;

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
            for advisory in &contracts {
                if advisory.is_emergency(&azero_connection).await? {
                    current_emergency_state = true;
                    if current_emergency_state != previous_emergency_state {
                        let current_block_number =
                            azero_connection.get_block_number_opt(None).await?;
                        warn!(
                        "Detected an emergency state at block {current_block_number:?} in the Advisory contract with an address {}",
                        advisory.address
                    );
                    }
                    break;
                }
            }

            if previous_emergency_state && !current_emergency_state {
                info!("Previously set emergency state has been lifted");
            }

            emergency.store(current_emergency_state, Ordering::Relaxed);

            // we sleep for about half a block production time before making another round of queries
            thread::sleep(Duration::from_millis(500))
        }
    }
}

pub async fn emergency_release(emergency: Arc<AtomicBool>) {
    while emergency.load(Ordering::Relaxed) {
        trace!("Event handling paused due to an emergency state in one of the advisory contracts")
    }
}
