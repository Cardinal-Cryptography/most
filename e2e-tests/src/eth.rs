use std::{fs, sync::Arc};

use anyhow::{anyhow, Result};
use ethers::{
    contract::Contract,
    core::{
        abi::{Abi, Tokenize},
        k256::ecdsa::SigningKey,
        types::{Address, TransactionReceipt, TransactionRequest, H256, U256},
    },
    middleware::{Middleware, SignerMiddleware},
    providers::{Http, Provider, ProviderExt},
    signers::{coins_bip39::English, MnemonicBuilder, Wallet},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    config::Config,
    token::{get_token_address_by_symbol, TokenJson},
};

pub type SignedConnection = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;
pub type ContractInstance =
    ethers::contract::ContractInstance<Arc<SignedConnection>, SignedConnection>;

#[derive(Deserialize, Serialize)]
pub struct EthContractAddressesJson {
    #[serde(rename = "ethTokens")]
    pub eth_tokens: [TokenJson; 2],
    #[serde(rename = "alephTokens")]
    pub aleph_tokens: [TokenJson; 1],
    pub most: String,
}

#[derive(Deserialize, Serialize)]
pub struct EthContractAddresses {
    pub most: String,
    pub weth: String,
    pub wazero: String,
    pub usdt: String,
}

impl From<EthContractAddressesJson> for EthContractAddresses {
    fn from(eth_contract_addresses: EthContractAddressesJson) -> Self {
        Self {
            most: eth_contract_addresses.most,
            weth: get_token_address_by_symbol(&eth_contract_addresses.eth_tokens, "WETH"),
            usdt: get_token_address_by_symbol(&eth_contract_addresses.eth_tokens, "USDT"),
            wazero: get_token_address_by_symbol(&eth_contract_addresses.aleph_tokens, "wAZERO"),
        }
    }
}

pub fn contract_addresses_json(
    eth_contract_addresses_path: &str,
) -> Result<EthContractAddressesJson> {
    Ok(serde_json::from_str(&fs::read_to_string(
        eth_contract_addresses_path,
    )?)?)
}

pub fn contract_addresses(eth_contract_addresses_path: &str) -> Result<EthContractAddresses> {
    Ok(EthContractAddresses::from(contract_addresses_json(
        eth_contract_addresses_path,
    )?))
}

pub fn contract_abi(contract_metadata_path: &str) -> Result<Abi> {
    let metadata: Value = serde_json::from_str(&fs::read_to_string(contract_metadata_path)?)?;
    Ok(serde_json::from_value(metadata["abi"].clone())?)
}

pub async fn connection(node_http: &str) -> Result<Provider<Http>> {
    Provider::<Http>::try_connect(node_http)
        .await
        .map_err(|e| anyhow!("Cannot establish ETH connection: {:?}", e))
}

pub async fn signed_connection(
    node_http: &str,
    wallet: Wallet<SigningKey>,
) -> Result<SignedConnection> {
    let connection = Provider::<Http>::try_connect(node_http).await?;
    Ok(SignerMiddleware::new_with_provider_chain(connection, wallet).await?)
}

pub fn contract_from_deployed(
    address: Address,
    abi: Abi,
    signed_connection: &SignedConnection,
) -> Result<ContractInstance> {
    Ok(Contract::new(
        address,
        abi,
        Arc::new(signed_connection.clone()),
    ))
}

pub async fn call_contract_method<T: Tokenize>(
    contract: ContractInstance,
    method: &str,
    args: T,
) -> Result<TransactionReceipt> {
    let call = contract.method::<_, H256>(method, args)?;
    let pending_tx = call.send().await?;
    pending_tx
        .confirmations(1)
        .await?
        .ok_or(anyhow!("'approve' tx receipt not available."))
}

pub async fn send_ether(
    from: Address,
    to: Address,
    amount: U256,
    signed_connection: &SignedConnection,
) -> Result<TransactionReceipt> {
    let send_tx = TransactionRequest::new()
        .to(to)
        .value(amount)
        .from(from);
    signed_connection
        .send_transaction(send_tx, None)
        .await?
        .await?
        .ok_or(anyhow!("Send tx receipt not available."))
}

/*pub async fn get_erc20_balance_of(
    contract_address: String,
    owner: Address,
    connection: &SignedConnection,
) -> Result<U256> {
}*/

/*pub async fn get_eth_balance_of(
    owner: Address,
    connection: &SignedConnection,
) -> Result<U256> {

}*/

pub async fn create_signed_connection(config: &Config) -> Result<SignedConnection> {
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(&*config.eth_mnemonic)
        .index(config.eth_dev_account_index)?
        .build()?;

    signed_connection(&config.eth_node_http, wallet).await
}
