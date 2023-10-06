use std::sync::Arc;

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    utility::BlocksApi,
    AlephConfig, AsConnection,
};
use ethers::{
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::ProviderError,
};
use futures::StreamExt;
use log::info;
use subxt::{events::Events, utils::H256};
use thiserror::Error;

use crate::{
    config::Config,
    connections::{
        azero::SignedAzeroWsConnection,
        eth::{EthConnectionError, EthWsConnection, SignedEthWsConnection},
    },
    contracts::{AzeroContractError, Membrane, MembraneInstance},
    helpers::chunks,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroListenerError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("subxt error")]
    Subxt(#[from] subxt::Error),

    #[error("azero provider error")]
    Provider(#[from] ProviderError),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("eth connection error")]
    EthConnection(#[from] EthConnectionError),

    #[error("eth contract error")]
    EthContractListen(#[from] ContractError<EthWsConnection>),

    #[error("eth contract error")]
    EthContractTx(#[from] ContractError<SignedEthWsConnection>),

    #[error("no block found")]
    BlockNotFound,

    #[error("no tx receipt")]
    NoTxReceipt,
}

pub struct AzeroListener;

impl AzeroListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
    ) -> Result<(), AzeroListenerError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            ..
        } = &*config;

        let instance = MembraneInstance::new(azero_contract_address, azero_contract_metadata)?;
        let contracts = vec![&instance.contract];

        // subscribe to new events
        let connection = azero_connection.as_connection();
        let mut subscription = connection
            .as_client()
            .blocks()
            .subscribe_finalized()
            .await?;

        info!("subscribing to new events");

        while let Some(Ok(block)) = subscription.next().await {
            let events = block.events().await?;
            handle_events(
                Arc::clone(&eth_connection),
                &config,
                events,
                &contracts,
                block.number(),
                block.hash(),
            )
            .await?;
        }

        Ok(())
    }
}

async fn handle_events(
    eth_connection: Arc<SignedEthWsConnection>,
    config: &Config,
    events: Events<AlephConfig>,
    contracts: &[&ContractInstance],
    block_number: u32,
    block_hash: H256,
) -> Result<(), AzeroListenerError> {
    for event in translate_events(
        events.iter(),
        contracts,
        Some(BlockDetails {
            block_number,
            block_hash,
        }),
    ) {
        handle_event(Arc::clone(&eth_connection), config, event?).await?;
    }
    Ok(())
}

async fn handle_event(
    eth_connection: Arc<SignedEthWsConnection>,
    config: &Config,
    event: ContractEvent,
) -> Result<(), AzeroListenerError> {
    let Config {
        eth_contract_address,
        ..
    } = config;

    if let Some(name) = event.name {
        if name.eq("Flip") {
            info!("handling A0 contract event: {name}");

            let address = eth_contract_address.parse::<Address>()?;
            let contract = Membrane::new(address, eth_connection);

            // TODO forward transfer & vote

            // let call: ContractCall<SignedEthWsConnection, ()> = contract.flop();

            // let tx = call
            //     .send()
            //     .await?
            //     .await?
            //     .ok_or(AzeroListenerError::NoTxReceipt)?;

            // info!("eth tx confirmed: {tx:?}");
        }
    }
    Ok(())
}
