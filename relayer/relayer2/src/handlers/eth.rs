use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use ethers::{
    abi::EncodePackedError,
    core::types::Address,
    prelude::ContractError,
    providers::{Http, Middleware, Provider, ProviderError},
    types::BlockNumber,
    utils::keccak256,
};
use log::{debug, error, info, trace, warn};
use redis::{aio::Connection as RedisConnection, RedisError};
use thiserror::Error;
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{sleep, Duration},
};

use crate::{
    config::Config,
    connections::{
        azero::AzeroConnectionWithSigner,
        eth::EthConnection,
        redis_helpers::{read_first_unprocessed_block_number, write_last_processed_block},
    },
    contracts::{
        AzeroContractError, CrosschainTransferRequestFilter, Most, MostEvents, MostInstance,
    },
    helpers::concat_u8_arrays,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthHandlerError {
    // #[error("provider error")]
    // Provider(#[from] ProviderError),

    // #[error("error when parsing ethereum address")]
    // FromHex(#[from] rustc_hex::FromHexError),

    // #[error("contract error")]
    // Contract(#[from] ContractError<Provider<Http>>),
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),
    // #[error("error when creating an ABI data encoding")]
    // AbiEncode(#[from] EncodePackedError),

    // #[error("redis connection error")]
    // Redis(#[from] RedisError),
}

pub async fn handle_events(
    events: Vec<MostEvents>,
    config: &Config,
    azero_connection: &AzeroConnectionWithSigner,
) -> Result<(), EthHandlerError> {
    for event in events {
        if let MostEvents::CrosschainTransferRequestFilter(
            crosschain_transfer_event @ CrosschainTransferRequestFilter {
                committee_id,
                dest_token_address,
                amount,
                dest_receiver_address,
                request_nonce,
                ..
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
                &committee_id.as_u128().to_le_bytes(),
                &dest_token_address,
                &amount.as_u128().to_le_bytes(),
                &dest_receiver_address,
                &request_nonce.as_u128().to_le_bytes(),
            ]);

            trace!("event concatenated bytes: {bytes:?}");

            let request_hash = keccak256(bytes);
            debug!("hashed event encoding: {request_hash:?}");

            let contract = MostInstance::new(
                azero_contract_address,
                azero_contract_metadata,
                config.azero_ref_time_limit,
                config.azero_proof_size_limit,
            )?;

            // send vote
            contract
                .receive_request(
                    azero_connection,
                    request_hash,
                    committee_id.as_u128(),
                    dest_token_address,
                    amount.as_u128(),
                    dest_receiver_address,
                    request_nonce.as_u128(),
                )
                .await?;
        }
    }

    Ok(())
}
