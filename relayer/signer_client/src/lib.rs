use futures::{SinkExt as _, StreamExt as _};
use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use subxt::ext::{sp_core::crypto::AccountId32, sp_runtime::MultiSignature};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_vsock::{OwnedReadHalf, OwnedWriteHalf, VsockStream};
use vsock::VsockAddr;

type EthAddress = ethers::types::Address;
type EthSignature = ethers::types::Signature;
type EthH256 = ethers::types::H256;
type EthTypedTransaction = ethers::types::transaction::eip2718::TypedTransaction;
type EthChainId = ethers::types::U64;

const ETH_MAINNET_CHAIN_ID: EthChainId = EthChainId::one();

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Invalid response from server")]
    InvalidResponse { expected: String, got: Response },
    #[error("Connection closed")]
    Closed,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Ping,
    AccountIdAzero,
    SignAzero {
        payload: Vec<u8>,
    },
    EthAddress,
    SignEthHash {
        hash: EthH256,
    },
    SignEthTx {
        tx: ethers::types::transaction::eip2718::TypedTransaction,
        chain_id: EthChainId,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Pong,
    AccountIdAzero {
        account_id: AccountId32,
    },
    SignedAzero {
        payload: Vec<u8>,
        signature: MultiSignature,
    },
    EthAddress {
        address: EthAddress,
    },
    SignedEthHash {
        hash: EthH256,
        signature: EthSignature,
    },
    SignedEthTx {
        tx: EthTypedTransaction,
        signature: EthSignature,
        chain_id: EthChainId,
    },
}

pub struct Client {
    read: FramedRead<OwnedReadHalf, LengthDelimitedCodec>,
    write: FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>,
}

impl From<VsockStream> for Client {
    fn from(connection: VsockStream) -> Self {
        let (read, write) = connection.into_split();
        let write = FramedWrite::new(write, LengthDelimitedCodec::new());
        let read = FramedRead::new(read, LengthDelimitedCodec::new());

        Self { write, read }
    }
}

impl Client {
    pub async fn new(cid: u32, port: u32) -> Result<Self, Error> {
        let connection = VsockStream::connect(VsockAddr::new(cid, port)).await?;
        Ok(connection.into())
    }

    pub async fn send<T: Serialize>(&mut self, msg: &T) -> Result<(), Error> {
        let msg = serde_json::to_vec(msg)?;
        self.write.send(msg.into()).await?;
        Ok(())
    }

    pub async fn recv<'de, T: Deserialize<'de>>(&mut self) -> Result<T, Error> {
        let msg = &self.read.next().await.ok_or(Error::Closed)??;
        let mut de = Deserializer::from_reader(msg.as_ref());
        let res = T::deserialize(&mut de)?;

        Ok(res)
    }

    pub async fn azero_account_id(&mut self) -> Result<AccountId32, Error> {
        self.send(&Command::AccountIdAzero).await?;

        match self.recv().await? {
            Response::AccountIdAzero { account_id } => Ok(account_id),
            other => Err(Error::InvalidResponse {
                expected: "AccountIdAzero".to_string(),
                got: other,
            }),
        }
    }

    pub async fn sign_azero(&mut self, payload: &[u8]) -> Result<MultiSignature, Error> {
        self.send(&Command::SignAzero {
            payload: payload.to_vec(),
        })
        .await?;

        match self.recv().await? {
            Response::SignedAzero {
                payload: return_payload,
                signature,
            } if return_payload == payload => Ok(signature),
            other => Err(Error::InvalidResponse {
                expected: format!("SignedAzero(payload: {:?})", payload),
                got: other,
            }),
        }
    }

    pub async fn eth_address(&mut self) -> Result<EthAddress, Error> {
        self.send(&Command::EthAddress).await?;

        match self.recv().await? {
            Response::EthAddress { address } => Ok(address),
            other => Err(Error::InvalidResponse {
                expected: "EthAddress".to_string(),
                got: other,
            }),
        }
    }

    pub async fn sign_eth_hash(&mut self, hash: EthH256) -> Result<EthSignature, Error> {
        self.send(&Command::SignEthHash { hash }).await?;

        match self.recv().await? {
            Response::SignedEthHash {
                hash: return_hash,
                signature,
            } if return_hash == hash => Ok(signature),
            other => Err(Error::InvalidResponse {
                expected: format!("SignedEthHash(hash: {:?})", hash),
                got: other,
            }),
        }
    }

    pub async fn sign_eth_tx(&mut self, tx: &EthTypedTransaction) -> Result<EthSignature, Error> {
        let chain_id = tx.chain_id().unwrap_or(ETH_MAINNET_CHAIN_ID);
        self.send(&Command::SignEthTx {
            tx: tx.clone(),
            chain_id,
        })
        .await?;
        let res = self.recv::<Response>().await?;

        if let Response::SignedEthTx {
            tx: mut return_tx,
            signature,
            chain_id: return_chain_id,
        } = res.clone()
        {
            // The Serialize and Deserialize implementations for TypedTransacion do not
            // serialize and deserialize the chain_id field, so we need to supply it
            // manually to the comparison here.
            if tx.chain_id().is_some() {
                return_tx.set_chain_id(return_chain_id);
            }

            if return_tx == *tx {
                return Ok(signature);
            }
        }

        Err(Error::InvalidResponse {
            expected: format!("SignedEthTx(tx: {:?})", tx),
            got: res,
        })
    }
}
