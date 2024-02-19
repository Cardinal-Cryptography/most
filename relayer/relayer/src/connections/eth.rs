use std::sync::{Arc, Mutex};

use ethers::{
    abi::Address,
    core::{types::H256, utils::hash_message},
    prelude::{
        nonce_manager::NonceManagerError, signer::SignerMiddlewareError, MiddlewareBuilder,
        NonceManagerMiddleware, SignerMiddleware,
    },
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer},
    types::{
        transaction::{eip2718::TypedTransaction, eip712::Eip712},
        Signature,
    },
};
use thiserror::Error;
use tokio::task::spawn_blocking;

pub type EthConnection = Provider<Http>;
pub type SignedEthConnection =
    SignerMiddleware<NonceManagerMiddleware<EthConnection>, EthereumSigner>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("Signer error")]
    SignerMiddleware(
        #[from] SignerMiddlewareError<NonceManagerMiddleware<EthConnection>, EthereumSigner>,
    ),

    #[error("Nonce manager error")]
    NonceManager(#[from] NonceManagerError<Provider<Http>>),

    #[error("Join error {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Signer client error {0}")]
    SignerClient(#[from] signer_client::Error),
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthereumSignerError {
    #[error("Local wallet error {0}")]
    LocalWallet(#[from] ethers::signers::WalletError),

    #[error("Vsock signer error {0}")]
    Vsock(#[from] EthVsockSignerError),
}

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthVsockSignerError {
    #[error("Join error {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Signer client error {0}")]
    SignerClient(#[from] signer_client::Error),

    #[error("Eip712 error {0}")]
    Eip712(String),
}

#[derive(Debug)]
pub enum EthereumSigner {
    Local(LocalWallet),
    Vsock(EthVsockSigner),
}

#[async_trait::async_trait]
impl Signer for EthereumSigner {
    type Error = EthereumSignerError;

    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error> {
        match self {
            EthereumSigner::Local(wallet) => Ok(wallet.sign_message(message).await?),
            EthereumSigner::Vsock(signer) => Ok(signer.sign_message(message).await?),
        }
    }

    async fn sign_transaction(&self, tx: &TypedTransaction) -> Result<Signature, Self::Error> {
        match self {
            EthereumSigner::Local(wallet) => Ok(wallet.sign_transaction(tx).await?),
            EthereumSigner::Vsock(signer) => Ok(signer.sign_transaction(tx).await?),
        }
    }

    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error> {
        match self {
            EthereumSigner::Local(wallet) => Ok(wallet.sign_typed_data(payload).await?),
            EthereumSigner::Vsock(signer) => Ok(signer.sign_typed_data(payload).await?),
        }
    }

    fn address(&self) -> Address {
        match self {
            EthereumSigner::Local(wallet) => wallet.address(),
            EthereumSigner::Vsock(signer) => signer.address(),
        }
    }

    fn chain_id(&self) -> u64 {
        match self {
            EthereumSigner::Local(wallet) => wallet.chain_id(),
            EthereumSigner::Vsock(signer) => signer.chain_id(),
        }
    }

    fn with_chain_id<T: Into<u64>>(self, chain_id: T) -> Self {
        match self {
            EthereumSigner::Local(wallet) => EthereumSigner::Local(wallet.with_chain_id(chain_id)),
            EthereumSigner::Vsock(signer) => EthereumSigner::Vsock(signer.with_chain_id(chain_id)),
        }
    }
}

#[derive(Debug)]
pub struct EthVsockSigner {
    client: Arc<Mutex<signer_client::Client>>,
    chain_id: u64,
    address: Address,
}

impl EthVsockSigner {
    async fn sign_hash(&self, hash: H256) -> Result<Signature, EthVsockSignerError> {
        let client = self.client.clone();
        let signature = spawn_blocking(move || {
            let client = client.lock().unwrap();
            client.sign_eth_hash(hash)
        })
        .await??;
        Ok(signature)
    }
}

#[async_trait::async_trait]
impl Signer for EthVsockSigner {
    type Error = EthVsockSignerError;

    async fn sign_message<S: Send + Sync + AsRef<[u8]>>(
        &self,
        message: S,
    ) -> Result<Signature, Self::Error> {
        let message = message.as_ref();
        let message_hash = hash_message(message);

        self.sign_hash(message_hash).await
    }

    async fn sign_transaction(&self, tx: &TypedTransaction) -> Result<Signature, Self::Error> {
        let client = self.client.clone();
        let tx = tx.clone();
        let signature = spawn_blocking(move || {
            let client = client.lock().unwrap();
            client.sign_eth_tx(&tx)
        })
        .await??;
        Ok(signature)
    }

    async fn sign_typed_data<T: Eip712 + Send + Sync>(
        &self,
        payload: &T,
    ) -> Result<Signature, Self::Error> {
        let encoded = payload
            .encode_eip712()
            .map_err(|e| EthVsockSignerError::Eip712(e.to_string()))?;

        self.sign_hash(H256::from(encoded)).await
    }

    fn address(&self) -> Address {
        self.address
    }

    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn with_chain_id<T: Into<u64>>(mut self, chain_id: T) -> Self {
        self.chain_id = chain_id.into();
        self
    }
}

pub async fn connect(url: &str) -> EthConnection {
    Provider::<Http>::connect(url).await
}

pub async fn with_local_wallet(
    connection: EthConnection,
    wallet: LocalWallet,
) -> Result<SignedEthConnection, EthConnectionError> {
    let nonce_manager = with_nonce_manager(connection, wallet.address()).await?;
    let signer = EthereumSigner::Local(wallet);

    Ok(SignerMiddleware::new_with_provider_chain(nonce_manager, signer).await?)
}

pub async fn with_signer(
    connection: EthConnection,
    cid: u32,
    port: u32,
) -> Result<SignedEthConnection, EthConnectionError> {
    let client = signer_client::Client::new(cid, port)?;
    let client = Arc::new(Mutex::new(client));
    let client_rc = client.clone();
    let address = spawn_blocking(move || {
        let client = client_rc.lock().unwrap();
        client.eth_address()
    })
    .await??;
    let nonce_manager = with_nonce_manager(connection, address).await?;

    let signer = EthVsockSigner {
        client,
        chain_id: 0,
        address,
    };
    let signer = EthereumSigner::Vsock(signer);

    Ok(SignerMiddleware::new_with_provider_chain(nonce_manager, signer).await?)
}

async fn with_nonce_manager(
    connection: EthConnection,
    address: Address,
) -> Result<NonceManagerMiddleware<Provider<Http>>, EthConnectionError> {
    let nonce_manager = connection.nonce_manager(address);
    nonce_manager.initialize_nonce(None).await?;

    Ok(nonce_manager)
}
