use std::{fs, sync::Arc};

use anyhow;
use ethers::{
    contract::{Contract, ContractInstance},
    core::{abi::Abi, k256::ecdsa::SigningKey, types::Address},
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderExt},
    signers::Wallet,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
pub struct EthContractAddresses {
    pub governance: String,
    pub migrations: String,
    pub most: String,
    pub usdt: String,
    pub weth9: String,
}

pub async fn signed_connection(
    node_http: &str,
    wallet: Wallet<SigningKey>,
) -> anyhow::Result<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    let connection = Provider::<Http>::try_connect(node_http).await?;
    Ok(SignerMiddleware::new_with_provider_chain(connection, wallet).await?)
}

pub fn contract_addresses(
    eth_contract_addresses_path: &str,
) -> anyhow::Result<EthContractAddresses> {
    Ok(serde_json::from_str(&fs::read_to_string(
        eth_contract_addresses_path,
    )?)?)
}

pub fn contract_abi(contract_metadata_path: &str) -> anyhow::Result<Abi> {
    let metadata: Value = serde_json::from_str(&fs::read_to_string(contract_metadata_path)?)?;
    Ok(serde_json::from_value(metadata["abi"].clone())?)
}

pub fn contract_from_deployed(
    address: Address,
    abi: Abi,
    signed_connection: SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
) -> anyhow::Result<
    ContractInstance<
        Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
        SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    >,
> {
    Ok(Contract::new(
        address,
        abi,
        Arc::new(signed_connection.clone()),
    ))
}
