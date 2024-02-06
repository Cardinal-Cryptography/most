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
            advisory_contract_address,
            ..
        } = &*config;

        let advisory_instance = AdvisoryInstance::new(
            &advisory_contract_address.clone().expect("Advisory address"),
            advisory_contract_metadata,
        )?;

        let is_emergency = advisory_instance.is_emergency(&azero_connection).await?;
        debug!("Advisory emergency state: {is_emergency}");
        emergency.store(is_emergency, Ordering::Relaxed);

        // let connection = azero_connection.as_connection();
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
                &[&advisory_instance.contract],
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
        advisory_contract_address,
        ..
    } = config;

    if event.contract.to_string().eq(&advisory_contract_address
        .clone()
        .expect("Advisory contract address"))
        && ["EmergencyChanged"].contains(&event.name.clone().unwrap().as_str())
    {
        debug!("Raw Advisory contract event: {event:?}");

        let decoded_event @ EmergencyChangedEvent { new_state, .. } =
            read_emergency_changed_event_data(&event.data)?;

        debug!("Decoded Advisory contract event: {decoded_event:?}");

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
