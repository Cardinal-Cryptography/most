use std::fs;

use aleph_client::{Connection, KeyPair, SignedConnection};
use anyhow;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct AzeroContractAddresses {
    pub governance: String,
    pub most: String,
    pub weth: String,
    pub test_oracle: String,
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

pub fn sign_connection(connection: &Connection, keypair: &KeyPair) -> SignedConnection {
    let signer = KeyPair::new(keypair.signer().clone());
    SignedConnection::from_connection(connection.clone(), signer)
}
