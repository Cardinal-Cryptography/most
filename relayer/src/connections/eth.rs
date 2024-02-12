use ethers::{
    prelude::{
        k256::ecdsa::SigningKey, nonce_manager::NonceManagerError, signer::SignerMiddlewareError,
        MiddlewareBuilder, NonceManagerMiddleware, SignerMiddleware,
    },
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer, Wallet},
};
use thiserror::Error;

pub type EthConnection = Provider<Http>;
pub type SignedEthConnection = SignerMiddleware<NonceManagerMiddleware<EthConnection>, LocalWallet>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("Signer error")]
    SignerMiddleware(
        #[from] SignerMiddlewareError<NonceManagerMiddleware<EthConnection>, Wallet<SigningKey>>,
    ),

    #[error("Nonce manager error")]
    NonceManager(#[from] NonceManagerError<Provider<Http>>),
}

pub async fn connect(url: &str) -> EthConnection {
    Provider::<Http>::connect(url).await
}

pub async fn sign(
    connection: EthConnection,
    wallet: LocalWallet,
) -> Result<SignedEthConnection, EthConnectionError> {
    let nonce_manager = connection.nonce_manager(wallet.address());
    nonce_manager.initialize_nonce(None).await?;

    Ok(SignerMiddleware::new_with_provider_chain(nonce_manager, wallet).await?)
}
