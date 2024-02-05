use ethers::{
    prelude::{k256::ecdsa::SigningKey, signer::SignerMiddlewareError, SignerMiddleware},
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Wallet},
};
use thiserror::Error;

pub type EthConnection = Provider<Http>;
pub type SignedEthConnection = SignerMiddleware<EthConnection, LocalWallet>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("middleware error")]
    SignerMiddleware(#[from] SignerMiddlewareError<EthConnection, Wallet<SigningKey>>),
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
