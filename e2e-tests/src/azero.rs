use std::fs;

use aleph_client::{Connection, KeyPair, SignedConnection};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AzeroContractAddressesJson {
    pub most: String,
    pub tokens: [[String; 3]; 2],
    pub oracle: String,
}

pub struct AzeroContractAddresses {
    pub most: String,
    pub weth: String,
    pub oracle: String,
}

impl From<AzeroContractAddressesJson> for AzeroContractAddresses {
    fn from(azero_contract_addresses: AzeroContractAddressesJson) -> Self {
        Self {
            most: azero_contract_addresses.most,
            weth: azero_contract_addresses.tokens[0][2].clone(),
            oracle: azero_contract_addresses.oracle,
        }
    }
}

pub fn contract_addresses_json(
    azero_contract_addresses_path: &str,
) -> anyhow::Result<AzeroContractAddressesJson> {
    Ok(serde_json::from_str(&fs::read_to_string(
        azero_contract_addresses_path,
    )?)?)
}

pub fn contract_addresses(
    azero_contract_addresses_path: &str,
) -> anyhow::Result<AzeroContractAddresses> {
    Ok(AzeroContractAddresses::from(contract_addresses_json(
        azero_contract_addresses_path,
    )?))
}

pub async fn connection(url: &str) -> Connection {
    Connection::new(url).await
}

pub async fn signed_connection(url: &str, keypair: &KeyPair) -> SignedConnection {
    SignedConnection::from_connection(
        Connection::new(url).await,
        KeyPair::new(keypair.signer().clone()),
    )
}

pub fn bytes32_to_string(data: &[u8; 32]) -> String {
    "0x".to_string() + &hex::encode(data)
}
