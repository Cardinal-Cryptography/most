use std::{
    str,
    str::{FromStr, Utf8Error},
};

use aleph_client::{contract::ContractInstance, AccountId, Balance, SignedConnection, TxInfo};
use thiserror::Error;

use crate::helpers::pad_zeroes;

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

    pub async fn receive_request(
        &self,
        signed_connection: &SignedConnection,
        dest_token_address: [u8; 32],
        dest_token_amount: u128,
        dest_receiver_address: [u8; 32],
        request_hash: [u8; 32],
    ) -> Result<TxInfo, AzeroContractError> {
        Ok(self
            .contract
            .contract_exec(
                signed_connection,
                "receive_request",
                &[
                    str::from_utf8(&dest_token_address)?,
                    &dest_token_amount.to_string(),
                    str::from_utf8(&dest_receiver_address)?,
                    str::from_utf8(&request_hash)?,
                ],
            )
            .await?)
    }
}
