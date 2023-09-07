use std::str::FromStr;

use aleph_client::{contract::ContractInstance, AccountId};
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum ContractsError {
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
    pub fn new(address: &str, metadata: &str) -> Result<Self, ContractsError> {
        let address = AccountId::from_str(address)
            .map_err(|why| ContractsError::NotAccountId(why.to_string()))?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata)?,
        })
    }
}
