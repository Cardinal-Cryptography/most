use std::sync::Arc;

use ethers::{
    prelude::SignerMiddleware,
    providers::{Provider, ProviderError, Ws},
    signers::{LocalWallet, WalletError},
};
use thiserror::Error;

pub type EthWsConnection = Arc<Provider<Ws>>;
// pub type SignedEthWsConnection = SignerMiddleware<Provider<Ws>, LocalWallet>;
pub type SignedEthWsConnection = SignerMiddleware<EthWsConnection, LocalWallet>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("connection error")]
    Provider(#[from] ProviderError),

    #[error("wallet error")]
    Wallet(#[from] WalletError),
}

async fn connect(url: &str) -> Result<Provider<Ws>, EthConnectionError> {
    Ok(Provider::<Ws>::connect(url).await?)
}

pub async fn init(url: &str) -> Result<EthWsConnection, EthConnectionError> {
    Ok(Arc::new(connect(url).await?))
}

pub async fn sign(
    connection: EthWsConnection,
) -> Result<SignedEthWsConnection, EthConnectionError> {
    let wallet = LocalWallet::decrypt_keystore("/home/filip/CloudStation/aleph/membrane-bridge/0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/keystore/129b9daee478e7bc5edada471982e31fa7705622", "chaos555")?;

    Ok(SignerMiddleware::new(connection, wallet))
}
