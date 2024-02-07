use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    contract_transcode::Value,
    AlephConfig,
};
use futures::StreamExt;
use log::{debug, info};
use subxt::{events::Events, utils::H256};

use super::AzeroListenerError;
use crate::{
    config::Config,
    connections::azero::AzeroWsConnection,
    contracts::{decode_bool_field, AdvisoryInstance, AzeroContractError},
};

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

        let instances: Vec<&ContractInstance> = contracts.iter().map(|c| &c.contract).collect();

        let mut is_emergency: bool = false;
        for advisory in &contracts {
            if advisory.is_emergency(&azero_connection).await? {
                is_emergency = true;
                break;
            }
        }

        info!("Advisory emergency state: {is_emergency}");
        emergency.store(is_emergency, Ordering::Relaxed);

        let mut subscription = azero_connection
            .as_client()
            .blocks()
            .subscribe_finalized()
            .await?;

        info!("Subscribing to the Advisory events");

        while let Some(Ok(block)) = subscription.next().await {
            let events = block.events().await?;
            handle_events(
                &config,
                emergency.clone(),
                events,
                &instances,
                block.number(),
                block.hash(),
            )
            .await?;
        }

        Ok(())
    }
}

async fn handle_events(
    config: &Config,
    emergency: Arc<AtomicBool>,
    events: Events<AlephConfig>,
    contracts: &[&ContractInstance],
    block_number: u32,
    block_hash: H256,
) -> Result<(), AzeroListenerError> {
    for maybe_event in translate_events(
        events.iter(),
        contracts,
        Some(BlockDetails {
            block_number,
            block_hash,
        }),
    ) {
        match maybe_event {
            Ok(event) => handle_event(config, event, emergency.clone()).await?,
            Err(why) => {
                debug!("Error reading contract event: {why:?}")
            }
        };
    }

    Ok(())
}

#[derive(Debug)]
pub struct EmergencyChangedEvent {
    pub previous_state: bool,
    pub new_state: bool,
}

async fn handle_event(
    config: &Config,
    event: ContractEvent,
    emergency: Arc<AtomicBool>,
) -> Result<(), AzeroListenerError> {
    let Config {
        advisory_contract_addresses,
        ..
    } = config;

    if advisory_contract_addresses
        .as_ref()
        .expect("Advisory contract addresses")
        .contains(&event.contract.to_string())
        && ["EmergencyChanged"].contains(&event.name.clone().unwrap().as_str())
    {
        debug!("Raw Advisory contract event: {event:?}");

        let decoded_event @ EmergencyChangedEvent { new_state, .. } =
            read_emergency_changed_event_data(&event.data)?;

        info!("Decoded Advisory contract event: {decoded_event:?}");

        emergency.store(new_state, Ordering::Relaxed);
    }

    Ok(())
}

fn read_emergency_changed_event_data(
    data: &HashMap<String, Value>,
) -> Result<EmergencyChangedEvent, AzeroContractError> {
    Ok(EmergencyChangedEvent {
        previous_state: decode_bool_field(data, "previous_state")?,
        new_state: decode_bool_field(data, "new_state")?,
    })
}
