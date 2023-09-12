use ethers::{
    prelude::SignerMiddleware,
    providers::{Provider, ProviderError, Ws},
    signers::{LocalWallet, WalletError},
};
use thiserror::Error;

pub type EthWsConnection = Provider<Ws>;
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
    connect(url).await
}

pub async fn sign(
    connection: EthWsConnection,
    wallet: LocalWallet,
) -> Result<SignedEthWsConnection, EthConnectionError> {
    Ok(
        SignerMiddleware::new_with_provider_chain(connection, wallet)
            .await
            .unwrap(),
    )
}
