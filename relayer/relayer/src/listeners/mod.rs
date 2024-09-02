use tokio::sync::oneshot;

use crate::contracts::MostEvents;

mod advisory;
mod azero;
mod eth;

#[derive(Debug)]
pub struct EthMostEvents {
    pub from_block: u32,
    pub to_block: u32,
    pub events: Vec<MostEvents>,
    pub events_ack_sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct AzeroMostEvents {
    pub events: Vec<ContractEvent>,
    pub from_block: u32,
    pub to_block: u32,
    pub ack: oneshot::Sender<u32>,
}

pub use advisory::*;
pub use azero::*;
use azero_client::ContractEvent;
pub use eth::*;
