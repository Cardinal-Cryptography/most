use std::str::FromStr;

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
}

#[derive(Debug)]
pub struct FlipperInstance {
    pub contract: ContractInstance,
}

impl FlipperInstance {
    pub fn new(address: &str, metadata_path: &str) -> Result<Self, AzeroContractError> {
        let address = AccountId::from_str(address)
            .map_err(|why| ContractsError::NotAccountId(why.to_string()))?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    pub async fn flop(
        &self,
        signed_connection: &SignedConnection,
    ) -> Result<TxInfo, AzeroContractError> {
        Ok(self
            .contract
            .contract_exec0(signed_connection, "flop")
            .await?)
    }
}
