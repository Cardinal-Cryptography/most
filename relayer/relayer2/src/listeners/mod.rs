use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::EthConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

mod eth;

#[derive(Debug)]
pub enum Message {
    EthBlockEvents {
        events: Vec<MostEvents>,
        ack_sender: oneshot::Sender<()>,
    },
}

pub use eth::*;
use tokio::sync::oneshot;
