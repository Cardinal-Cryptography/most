use ethers::{
    prelude::{signer::SignerMiddlewareError, SignerMiddleware},
    providers::{Http, Provider, ProviderError, ProviderExt},
    signers::{LocalWallet, WalletError},
};
use thiserror::Error;

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

    #[error("signer error")]
    Signer(#[from] SignerMiddlewareError<EthConnection, LocalWallet>),
}

pub async fn connect(url: &str) -> EthConnection {
    Provider::<Http>::connect(url).await
}

pub async fn sign(
    connection: EthConnection,
    wallet: LocalWallet,
) -> Result<SignedEthConnection, EthConnectionError> {
    Ok(SignerMiddleware::new_with_provider_chain(connection, wallet).await?)
}
