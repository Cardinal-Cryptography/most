use std::{
    str,
    str::{FromStr, Utf8Error},
};

use aleph_client::{contract::ContractInstance, AccountId, SignedConnection, TxInfo};
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroContractError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("not account id")]
    NotAccountId(String),

    #[error("Invalid UTF-8 sequence")]
    InvalidUTF8(#[from] Utf8Error),
}

#[derive(Debug)]
pub struct MembraneInstance {
    pub contract: ContractInstance,
}

impl MembraneInstance {
    pub fn new(address: &str, metadata_path: &str) -> Result<Self, AzeroContractError> {
        let address = AccountId::from_str(address)
            .map_err(|why| AzeroContractError::NotAccountId(why.to_string()))?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn receive_request(
        &self,
        signed_connection: &SignedConnection,
        request_hash: [u8; 32],
        dest_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        Ok(self
            .contract
            .contract_exec(
                signed_connection,
                "receive_request",
                &[
                    bytes32_to_str(&request_hash),
                    bytes32_to_str(&dest_token_address),
                    amount.to_string(),
                    bytes32_to_str(&dest_receiver_address),
                    request_nonce.to_string(),
                ],
            )
            .await?)
    }
}

fn bytes32_to_str(data: &[u8; 32]) -> String {
    "0x".to_owned() + &hex::encode(data)
}
