use ethers::utils::keccak256;
use log::{debug, error, info, trace};
use thiserror::Error;

use crate::{
    config::Config,
    connections::azero::AzeroConnectionWithSigner,
    contracts::{AzeroContractError, CrosschainTransferRequestFilter, MostEvents, MostInstance},
    helpers::concat_u8_arrays,
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthHandlerError {
    #[error("azero contract error")]
    AzeroContract(#[from] AzeroContractError),
}

pub struct EthHandler;

impl EthHandler {
    pub async fn handle_event(
        event: MostEvents,
        config: &Config,
        azero_connection: &AzeroConnectionWithSigner,
    ) -> Result<(), EthHandlerError> {
        let Config {
            azero_contract_address,
            azero_contract_metadata,
            ..
        } = config;

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

        Ok(())
    }
}
