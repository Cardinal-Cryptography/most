use std::sync::Arc;

use ethers::providers::{Provider, ProviderError, Ws};
use thiserror::Error;

pub struct EthConnection;

pub type EthWsConnection = Arc<Provider<Ws>>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("connection error")]
    Provider(#[from] ProviderError),
}

impl EthConnection {
    pub async fn init(url: &str) -> Result<EthWsConnection, EthConnectionError> {
        Ok(Arc::new(Self::connect(url).await?))
    }

    async fn connect(url: &str) -> Result<Provider<Ws>, EthConnectionError> {
        Ok(Provider::<Ws>::connect(url).await?)
    }
}
