use std::sync::Arc;

use ethers::{
    prelude::SignerMiddleware,
    providers::{Provider, ProviderError, Ws},
    signers::LocalWallet,
    utils::Anvil,
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
}

async fn connect(url: &str) -> Result<Provider<Ws>, EthConnectionError> {
    Ok(Provider::<Ws>::connect(url).await?)
}

pub async fn init(url: &str) -> Result<EthWsConnection, EthConnectionError> {
    Ok(Arc::new(connect(url).await?))
}

pub async fn sign(connection: EthWsConnection) -> SignedEthWsConnection {
    let anvil = Anvil::new().spawn();
    let wallet: LocalWallet = anvil.keys()[0].clone().into();
    SignerMiddleware::new(connection, wallet)
}
