use std::sync::Arc;

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::ContractError,
    providers::{Middleware, Provider, ProviderError, StreamExt, Ws},
    utils::keccak256,
};
use log::{debug, info, trace};
use thiserror::Error;

use crate::{
    config::Config,
    connections::{azero::SignedAzeroWsConnection, eth::SignedEthWsConnection},
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Membrane, MembraneEvents,
        MembraneInstance,
    },
    helpers::concat_u8_arrays,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthListenerError {
    #[error("provider error")]
    Provider(#[from] ProviderError),

    #[error("error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("contract error")]
    Contract(#[from] ContractError<Provider<Ws>>),

    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("error when creating an ABI data encoding")]
    AbiEncode(#[from] EncodePackedError),
}

pub struct EthListener;

impl EthListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<SignedAzeroWsConnection>,
        eth_connection: Arc<SignedEthWsConnection>,
    ) -> Result<(), EthListenerError> {
        let Config {
            eth_contract_address,
            ..
        } = &*config;

        let address = eth_contract_address.parse::<Address>()?;
        let contract = Membrane::new(address, Arc::clone(&eth_connection));

        let last_block_number = eth_connection.get_block_number().await.unwrap().as_u32();

        let events = contract.events().from_block(last_block_number);
        let mut stream = events.stream().await.unwrap();

        info!("subscribing to new events");

        while let Some(Ok(event)) = stream.next().await {
            handle_event(&event, &config, Arc::clone(&azero_connection)).await?;
        }

        Ok(())
    }
}

async fn handle_event(
    event: &MembraneEvents,
    config: &Config,
    azero_connection: Arc<SignedAzeroWsConnection>,
) -> Result<(), EthListenerError> {
    if let MembraneEvents::CrosschainTransferRequestFilter(
        crosschain_transfer_event @ CrosschainTransferRequestFilter {
            dest_token_address,
            amount,
            dest_receiver_address,
            request_nonce,
        },
    ) = event
    {
        let Config {
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

        info!("handling eth contract event: {crosschain_transfer_event:?}");

        // concat bytes
        let bytes = concat_u8_arrays(vec![
            dest_token_address,
            &amount.as_u128().to_le_bytes(),
            dest_receiver_address,
            &request_nonce.as_u128().to_le_bytes(),
        ]);

        trace!("event concatenated bytes: {bytes:?}");

        let request_hash = keccak256(bytes);
        debug!("hashed event encoding: {request_hash:?}");

        let contract = MembraneInstance::new(azero_contract_address, azero_contract_metadata)?;

        // send vote
        contract
            .receive_request(
                &azero_connection,
                request_hash,
                *dest_token_address,
                amount.as_u128(),
                *dest_receiver_address,
                request_nonce.as_u128(),
            )
            .await?;
    }

    Ok(())
}
