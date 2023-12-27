use ethers::{
    prelude::SignerMiddleware,
    providers::{Provider, ProviderError, Http},
    signers::{LocalWallet, WalletError},
};
use thiserror::Error;
use ethers::providers::ProviderExt;

pub type EthConnection = Provider<Http>;
pub type SignedEthConnection = SignerMiddleware<EthConnection, LocalWallet>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("connection error")]
    Provider(#[from] ProviderError),

    #[error("wallet error")]
    Wallet(#[from] WalletError),
}

pub async fn connect(url: &str) -> EthConnection {
    Provider::<Http>::connect(url).await
}

pub async fn sign(
    connection: EthConnection,
    wallet: LocalWallet,
) -> Result<SignedEthConnection, EthConnectionError> {
    Ok(
        SignerMiddleware::new_with_provider_chain(connection, wallet)
            .await
            .unwrap(),
    )
}
