use std::error::Error;

use anyhow::{anyhow, bail, Result};
use contract_transcode::Value;
use subxt::{events::EventDetails, PolkadotConfig};

use crate::{AccountId, ContractEmitted, ContractEvent, ContractInstance};

/// Try to convert `events` to `ContractEvent` using matching contract from `contracts`.
pub fn translate_events<
    Err: Error + Into<anyhow::Error> + Send + Sync + 'static,
    E: Iterator<Item = Result<EventDetails<PolkadotConfig>, Err>>,
>(
    events: E,
    contracts: &[&ContractInstance],
) -> Vec<Result<ContractEvent>> {
    events
        .filter_map(|maybe_event| {
            maybe_event
                .map(|e| e.as_event::<ContractEmitted>().ok().flatten())
                .transpose()
        })
        .map(|maybe_event| match maybe_event {
            Ok(e) => translate_event(&e, contracts),
            Err(e) => Err(anyhow::Error::from(e)),
        })
        .collect()
}

/// Try to convert `event` to `ContractEvent` using matching contract from `contracts`.
fn translate_event(
    event: &ContractEmitted,
    contracts: &[&ContractInstance],
) -> Result<ContractEvent> {
    let matching_contract = contracts
        .iter()
        .find(|contract| contract.address() == &event.contract)
        .ok_or_else(|| anyhow!("The event wasn't emitted by any of the provided contracts"))?;

    let data = zero_prefixed(&event.data);
    let data = matching_contract
        .transcoder
        .decode_contract_event(&mut data.as_slice())?;

    build_event(matching_contract.address.clone(), data)
}

/// The contract transcoder assumes there is an extra byte (that it discards) indicating the size of the data. However,
/// data arriving through the subscription as used in this file don't have this extra byte. This function adds it.
fn zero_prefixed(data: &[u8]) -> Vec<u8> {
    let mut result = vec![0];
    result.extend_from_slice(data);
    result
}

fn build_event(address: AccountId, event_data: Value) -> Result<ContractEvent> {
    match event_data {
        Value::Map(map) => Ok(ContractEvent {
            contract: address,
            name: map.ident(),
            data: map
                .iter()
                .map(|(key, value)| (key.to_string(), value.clone()))
                .collect(),
        }),
        _ => bail!("Contract event data is not a map"),
    }
}
