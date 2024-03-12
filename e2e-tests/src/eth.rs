use std::{fs, sync::Arc};

use ethers::{
    contract::{Contract, ContractInstance},
    core::{
        abi::{Abi, Tokenize},
        k256::ecdsa::SigningKey,
        types::{Address, TransactionReceipt, TransactionRequest, H256, U256},
    },
    middleware::{Middleware, SignerMiddleware},
    providers::{Http, Provider, ProviderExt},
    signers::Wallet,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
pub struct EthContractAddresses {
    pub migrations: String,
    pub most: String,
    pub usdt: String,
    pub weth: String,
}

pub async fn connection(node_http: &str) -> anyhow::Result<Provider<Http>> {
    Provider::<Http>::try_connect(node_http)
        .await
        .map_err(|e| anyhow::anyhow!("Cannot establish ETH connection: {:?}", e))
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
    signed_connection: &SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
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

pub async fn call_contract_method<T: Tokenize>(
    contract: ContractInstance<
        Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
        SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
    >,
    method: &str,
    gas_limit: u128,
    args: T,
) -> anyhow::Result<TransactionReceipt> {
    let call = contract.method::<_, H256>(method, args)?;
    let call = call.gas(gas_limit);
    let pending_tx = call.send().await?;
    pending_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow::anyhow!("'approve' tx receipt not available."))
}
pub async fn send_tx(
    from: Address,
    to: Address,
    amount: U256,
    signed_connection: &SignerMiddleware<Provider<Http>, Wallet<SigningKey>>,
) -> anyhow::Result<TransactionReceipt> {
    let send_tx = TransactionRequest::new()
        .to(to)
        .value(U256::from(amount))
        .from(from);
    signed_connection
        .send_transaction(send_tx, None)
        .await?
        .await?
        .ok_or(anyhow::anyhow!("Send tx receipt not available."))
}
