use std::time::Duration;

use azero_client::{AccountId, ClientConfig, KeyPair, MultiSignature, Signer};
use signer_client::Client;
use subxt::ext::sp_core::Pair;
use tokio::sync::Mutex;

pub type AzeroWsConnection = azero_client::Client;

pub async fn init(url: &str) -> AzeroWsConnection {
    AzeroWsConnection::new(&ClientConfig {
        address: url.to_string(),
        backoff_millis: 1000,
        backoff_factor: 2,
        backoff_max_delay: Duration::from_secs(30),
    })
    .await
    .unwrap()
}

pub struct AzeroSignerClient {
    client: Mutex<Client>,
    account_id: AccountId,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Signer error: {0}")]
    Signer(#[from] signer_client::Error),

    #[error("Rpc error: {0}")]
    Rpc(#[from] subxt::error::Error),
}

impl AzeroSignerClient {
    pub async fn new(cid: u32, port: u32) -> Result<Self, Error> {
        let mut client = Client::new(cid, port).await?;
        let account_id = client.azero_account_id().await?;
        let client = Mutex::new(client);

        Ok(Self {
            client,
            account_id: account_id.into(),
        })
    }
}

pub enum AzeroSigner {
    Dev(Box<KeyPair>),
    Signer(AzeroSignerClient),
}

impl AzeroSigner {
    fn account_id(&self) -> &AccountId {
        match self {
            AzeroSigner::Dev(keypair) => keypair.account_id(),
            AzeroSigner::Signer(signer) => &signer.account_id,
        }
    }

    async fn sign(&self, payload: &[u8]) -> Result<MultiSignature, anyhow::Error> {
        match self {
            AzeroSigner::Dev(keypair) => Ok(keypair.signer().sign(payload).into()),
            AzeroSigner::Signer(signer) => {
                let mut client = signer.client.lock().await;
                let payload = payload.to_vec();
                let signature = client.sign_azero(&payload).await?;

                Ok(signature.into())
            }
        }
    }
}

#[async_trait::async_trait]
impl Signer for AzeroSigner {
    type Error = anyhow::Error;

    fn account_id(&self) -> &AccountId {
        AzeroSigner::account_id(&self)
    }

    async fn sign(&self, payload: &[u8]) -> Result<MultiSignature, Self::Error> {
        AzeroSigner::sign(&self, payload).await
    }
}
