use std::{fmt::Debug, sync::Arc, time::Duration};

use ethers::{
    abi::Address,
    core::{types::H256, utils::hash_message},
    middleware::Middleware,
    prelude::{
        gas_escalator::{Frequency, GeometricGasPrice},
        nonce_manager::NonceManagerError,
        signer::SignerMiddlewareError,
        BlockNumber, GasEscalatorMiddleware, MiddlewareBuilder, NonceManagerMiddleware,
        SignerMiddleware,
    },
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer},
    types::{
        transaction::{eip2718::TypedTransaction, eip712::Eip712},
        Signature,
    },
};
use log::{debug, warn};
use thiserror::Error;
use tokio::{sync::Mutex, time::sleep};

use crate::{config::Config, consts::ETH_BLOCK_PROD_TIME_SEC};

pub type EthConnection = Provider<Http>;
pub type GasEscalatingEthConnection = GasEscalatorMiddleware<EthConnection>;
pub type SignedEthConnection =
    SignerMiddleware<NonceManagerMiddleware<GasEscalatingEthConnection>, EthereumSigner>;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum EthConnectionError {
    #[error("Signer error")]
    SignerMiddleware(
        #[from]
        SignerMiddlewareError<NonceManagerMiddleware<GasEscalatingEthConnection>, EthereumSigner>,
    ),

    #[error("Nonce manager error")]
    NonceManager(#[from] NonceManagerError<GasEscalatingEthConnection>),

    #[error("Join error {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Signer client error {0}")]
    SignerClient(#[from] signer_client::Error),

    #[error("Local wallet error {0}")]
    LocalWallet(#[from] ethers::signers::WalletError),
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

pub struct EthVsockSigner {
    client: Mutex<signer_client::Client>,
    chain_id: u64,
    address: Address,
}

impl Debug for EthVsockSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EthVsockSigner")
            .field("chain_id", &self.chain_id)
            .field("address", &self.address)
            .finish()
    }
}

impl EthVsockSigner {
    async fn sign_hash(&self, hash: H256) -> Result<Signature, EthVsockSignerError> {
        let mut client = self.client.lock().await;
        let signature = client.sign_eth_hash(hash).await?;

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
        let mut client = self.client.lock().await;
        let tx = tx.clone();
        let signature = client.sign_eth_tx(&tx).await?;

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

pub async fn connect(config: &Config) -> EthConnection {
    Provider::<Http>::connect(&config.eth_node_http_url).await
}

pub async fn with_local_wallet(
    connection: GasEscalatingEthConnection,
    wallet: LocalWallet,
) -> Result<SignedEthConnection, EthConnectionError> {
    let nonce_manager = with_nonce_manager(connection, wallet.address()).await?;
    let signer = EthereumSigner::Local(wallet);

    Ok(SignerMiddleware::new_with_provider_chain(nonce_manager, signer).await?)
}

pub async fn with_signer(
    connection: GasEscalatingEthConnection,
    cid: u32,
    port: u32,
) -> Result<SignedEthConnection, EthConnectionError> {
    let mut client = signer_client::Client::new(cid, port).await?;
    let address = client.eth_address().await?;
    let client = Mutex::new(client);
    let nonce_manager = with_nonce_manager(connection, address).await?;

    let signer = EthVsockSigner {
        client,
        chain_id: 0,
        address,
    };
    let signer = EthereumSigner::Vsock(signer);

    Ok(SignerMiddleware::new_with_provider_chain(nonce_manager, signer).await?)
}

pub async fn with_nonce_manager(
    connection: GasEscalatingEthConnection,
    address: Address,
) -> Result<NonceManagerMiddleware<GasEscalatingEthConnection>, EthConnectionError> {
    let nonce_manager = connection.nonce_manager(address);
    nonce_manager.initialize_nonce(None).await?;

    Ok(nonce_manager)
}

pub async fn with_gas_escalator(connection: EthConnection) -> GasEscalatingEthConnection {
    let escalator = GeometricGasPrice::new(1.125, 25u64, None::<u64>);
    GasEscalatorMiddleware::new(connection, escalator, Frequency::Duration(15000))
}

#[cfg(feature = "l2")]
pub async fn get_next_finalized_block_number(
    eth_connection: Arc<EthConnection>,
    not_older_than: u32,
) -> u32 {
    // In L2 context we treat latest block as finalized.
    get_block_not_older_than(eth_connection, not_older_than, BlockNumber::Latest).await
}

#[cfg(not(feature = "l2"))]
pub async fn get_next_finalized_block_number(
    eth_connection: Arc<EthConnection>,
    not_older_than: u32,
) -> u32 {
    // In ethereum l1 context we treat finalized block as, well, finalized :).
    get_block_not_older_than(eth_connection, not_older_than, BlockNumber::Finalized).await
}

pub async fn get_block_not_older_than(
    eth_connection: Arc<EthConnection>,
    not_older_than: u32,
    block: BlockNumber,
) -> u32 {
    loop {
        match eth_connection.get_block(block).await {
            Ok(Some(block)) => {
                let block_number = block.number.expect("Block has a number.").as_u32();
                if block_number >= not_older_than {
                    return block_number;
                }
            }
            Ok(None) => {
                warn!("No block found.");
            }
            Err(e) => {
                warn!("Client error when getting block: {e}");
            }
        };

        debug!("Waiting for a next block");
        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;
    }
}
