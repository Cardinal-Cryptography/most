// Context of this file consists mostly of a modified copies of code snippets taken from ink repository on github,
// as there is no equivalent support in version 4.3

// Copyright (C) Parity Technologies (UK) Ltd.
// Modified by Cardinal Cryptography 2023.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(feature = "std")]
use std::fmt::Debug;

use ink::env::{DefaultEnvironment, Environment};
use ink_e2e::{PolkadotConfig, H256};
use scale::{Decode, Encode};
use subxt::{
    blocks::ExtrinsicEvents,
    events::StaticEvent,
    ext::{scale_decode, scale_encode},
};

/// A decoded event with its associated topics.
pub struct EventWithTopics<T> {
    pub topics: Vec<H256>,
    pub event: T,
}

#[derive(Decode, Encode, scale_decode::DecodeAsType, scale_encode::EncodeAsType, Debug)]
#[decode_as_type(trait_bounds = "", crate_path = "subxt::ext::scale_decode")]
#[encode_as_type(crate_path = "subxt::ext::scale_encode")]
/// A custom event emitted by the contract.
pub struct ContractEmitted {
    pub contract: <DefaultEnvironment as Environment>::AccountId,
    pub data: Vec<u8>,
}

impl StaticEvent for ContractEmitted {
    const PALLET: &'static str = "Contracts";
    const EVENT: &'static str = "ContractEmitted";
}

/// Returns all the `ContractEmitted` events emitted by the contract.
pub fn get_contract_emitted_events(
    extrinsic_events: ExtrinsicEvents<PolkadotConfig>,
) -> Result<Vec<EventWithTopics<ContractEmitted>>, subxt::Error> {
    let mut events_with_topics = Vec::new();
    for event in extrinsic_events.iter() {
        let event = event?;
        if let Some(decoded_event) = event.as_event::<ContractEmitted>()? {
            let event_with_topics = EventWithTopics {
                event: decoded_event,
                topics: event.topics().iter().cloned().map(Into::into).collect(),
            };
            events_with_topics.push(event_with_topics);
        }
    }
    Ok(events_with_topics)
}

// Decodes all the `ContractEmitted` events emitted by the contract as a selected type.
// Omits events that cannot be decoded.
pub fn filter_decode_events_as<E: Decode>(
    events_with_topics: Vec<EventWithTopics<ContractEmitted>>,
) -> Vec<E> {
    events_with_topics
        .iter()
        .filter_map(|event| E::decode(&mut &event.event.data[..]).ok())
        .collect()
}
