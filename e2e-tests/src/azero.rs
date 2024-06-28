use std::fs;

use aleph_client::{Connection, KeyPair, SignedConnection};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::token::{get_token_address_by_symbol, TokenJson};

#[derive(Deserialize, Serialize)]
pub struct AzeroContractAddressesJson {
    pub most: String,
    pub oracle: String,
    pub advisory: String,
    #[serde(rename = "ethTokens")]
    pub eth_tokens: [TokenJson; 2],
    #[serde(rename = "alephTokens")]
    pub aleph_tokens: [TokenJson; 1],
}

pub struct AzeroContractAddresses {
    pub most: String,
    pub weth: String,
    pub wazero: String,
    pub usdt: String,
}

impl From<AzeroContractAddressesJson> for AzeroContractAddresses {
    fn from(azero_contract_addresses: AzeroContractAddressesJson) -> Self {
        Self {
            most: azero_contract_addresses.most,
            weth: get_token_address_by_symbol(&azero_contract_addresses.eth_tokens, "WETH"),
            usdt: get_token_address_by_symbol(&azero_contract_addresses.eth_tokens, "USDT"),
            wazero: get_token_address_by_symbol(&azero_contract_addresses.aleph_tokens, "wAZERO"),
        }
    }
}

pub fn contract_addresses_json(
    azero_contract_addresses_path: &str,
) -> Result<AzeroContractAddressesJson> {
    Ok(serde_json::from_str(&fs::read_to_string(
        azero_contract_addresses_path,
    )?)?)
}

pub fn contract_addresses(azero_contract_addresses_path: &str) -> Result<AzeroContractAddresses> {
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

/*pub async fn get_psp22_balance_of(
    connection: &Connection,
    token_address: &str,
    account_address: &str,
) -> Result<u128> {
}

pub async fn get_azero_balance_of(
    connection: &Connection,
    token_address: &str,
    account_address: &str,
) -> Result<u128> {
}*/
