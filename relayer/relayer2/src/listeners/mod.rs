use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::EthConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

mod advisory;
mod eth;

#[derive(Debug)]
pub struct EthMostEvents {
    pub events: Vec<MostEvents>,
    pub ack_sender: oneshot::Sender<()>,
}

pub use eth::*;
use tokio::sync::oneshot;
