use aleph_client::{contract::event::ContractEvent, AlephConfig};
use subxt::{events::Events, utils::H256};
use tokio::sync::oneshot;

use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::EthConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

mod advisory;
mod azero;
mod eth;

#[derive(Debug)]
pub struct EthMostEvent {
    pub event: MostEvents,
    pub event_ack_sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct EthMostEvents {
    pub events: Vec<MostEvents>,
    pub events_ack_sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct AzeroMostEvents {
    pub events: Vec<ContractEvent>,
    pub events_ack_sender: oneshot::Sender<()>,
}

pub use advisory::*;
pub use azero::*;
pub use eth::*;
