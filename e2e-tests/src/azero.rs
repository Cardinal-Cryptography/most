use std::fs;

use aleph_client::{Connection, KeyPair, SignedConnection};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AzeroContractAddresses {
    pub most: String,
    pub weth: String,
    pub oracle: String,
}

pub fn contract_addresses(
    azero_contract_addresses_path: &str,
) -> anyhow::Result<AzeroContractAddresses> {
    Ok(serde_json::from_str(&fs::read_to_string(
        azero_contract_addresses_path,
    )?)?)
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
