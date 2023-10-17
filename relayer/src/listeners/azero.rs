use std::{collections::HashMap, sync::Arc};

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    contract_transcode::Value,
    AlephConfig, AsConnection,
};
use ethers::{
    abi::{self, EncodePackedError, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::ProviderError,
    types::U256,
    utils::keccak256,
};
use futures::StreamExt;
use log::{debug, info, trace};
use subxt::{events::Events, utils::H256};
use thiserror::Error;

use crate::{
    config::Config,
    connections::{
        azero::SignedAzeroWsConnection,
        eth::{EthConnectionError, EthWsConnection, SignedEthWsConnection},
    },
    contracts::{AzeroContractError, Membrane, MembraneInstance},
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

    #[error("missing data from event")]
    MissingEventData(String),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),

    #[error("unexpected error")]
    Unexpected,
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

fn get_event_data(
    data: &HashMap<String, Value>,
    field: &str,
) -> Result<[u8; 32], AzeroListenerError> {
    match data.get(field) {
        Some(Value::Hex(hex)) => {
            let mut result = [0u8; 32];
            result.copy_from_slice(hex.bytes());
            Ok(result)
        }
        _ => Err(AzeroListenerError::Unexpected),
    }
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

    if let Some(name) = &event.name {
        if name.eq("CrosschainTransferRequest") {
            info!("handling A0 contract event: {event:?}");

            let data = event.data;

            // decode event data
            let dest_token_address = get_event_data(&data, "dest_token_address")?;
            let amount = get_event_data(&data, "amount")?;
            let dest_receiver_address = get_event_data(&data, "dest_receiver_address")?;
            let request_nonce = get_event_data(&data, "request_nonce")?;

            let amount = U256::from_little_endian(&amount);
            let request_nonce = U256::from_little_endian(&request_nonce);

            let bytes = abi::encode_packed(&[
                Token::FixedBytes(dest_token_address.to_vec()),
                Token::Int(amount),
                Token::FixedBytes(dest_receiver_address.to_vec()),
                Token::Int(request_nonce),
            ])?;

            trace!("ABI event encoding: {bytes:?}");

            // hash event data

            let request_hash = keccak256(bytes);
            debug!("hashed event encoding: {request_hash:?}");

            let address = eth_contract_address.parse::<Address>()?;
            let contract = Membrane::new(address, eth_connection);

            //  forward transfer & vote

            let call: ContractCall<SignedEthWsConnection, ()> = contract.receive_request(
                request_hash,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
            );

            let tx = call
                .send()
                .await?
                .await?
                .ok_or(AzeroListenerError::NoTxReceipt)?;

            info!("eth tx confirmed: {tx:?}");
        }
    }
    Ok(())
}
