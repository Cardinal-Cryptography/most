use std::str::FromStr;

use aleph_client::{contract::ContractInstance, AccountId};
use thiserror::Error;

use crate::config::Config;

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
    contract: ContractInstance,
}

impl FlipperInstance {
    pub fn new(config: &Config) -> Result<Self, ContractsError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            ..
        } = &*config;

        let address = AccountId::from_str(&azero_contract_address)
            .map_err(|why| ContractsError::NotAccountId(why.to_string()))?;

        Ok(Self {
            contract: ContractInstance::new(address, &azero_contract_metadata)?,
        })
    }
}
