use aleph_client::contract::event::ContractEvent;
use tokio::sync::oneshot;

use crate::contracts::MostEvents;

mod advisory;
mod azero;
mod eth;

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
