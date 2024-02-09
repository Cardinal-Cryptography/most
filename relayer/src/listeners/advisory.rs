use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

use log::{info, warn};

use super::AzeroListenerError;
use crate::{config::Config, connections::azero::AzeroWsConnection, contracts::AdvisoryInstance};

pub struct AdvisoryListener;

impl AdvisoryListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        emergency: Arc<AtomicBool>,
    ) -> Result<(), AzeroListenerError> {
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
                |mut acc, address| -> Result<Vec<AdvisoryInstance>, AzeroListenerError> {
                    acc.push(AdvisoryInstance::new(&address, advisory_contract_metadata)?);
                    Ok(acc)
                },
            )?;

        loop {
            let mut is_emergency: bool = false;
            for advisory in &contracts {
                if advisory.is_emergency(&azero_connection).await? {
                    is_emergency = true;
                    break;
                }
            }

            match is_emergency {
                true => warn!("Advisory emergency state: {is_emergency}"),
                false => info!("Advisory emergency state: {is_emergency}"),
            }

            emergency.store(is_emergency, Ordering::Relaxed);
            thread::sleep(Duration::from_millis(500))
        }
    }
}
