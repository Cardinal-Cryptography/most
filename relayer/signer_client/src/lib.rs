use serde::{Deserialize, Serialize};
use serde_json::Deserializer;
use subxt::ext::{sp_core::crypto::AccountId32, sp_runtime::MultiSignature};
use vsock::VsockStream;

type EthAddress = ethers::types::Address;
type EthSignature = ethers::types::Signature;
type EthH256 = ethers::types::H256;
type EthTypedTransaction = ethers::types::transaction::eip2718::TypedTransaction;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Invalid response from server")]
    InvalidResponse { expected: String, got: Response },
}

#[derive(Serialize, Deserialize, Debug)]
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
    },
}

#[derive(Serialize, Deserialize, Debug)]
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
    },
}

#[derive(Debug)]
pub struct Client {
    connection: VsockStream,
}

impl From<VsockStream> for Client {
    fn from(connection: VsockStream) -> Self {
        Self { connection }
    }
}

impl Client {
    pub fn new(cid: u32, port: u32) -> Result<Self, Error> {
        let connection = VsockStream::connect_with_cid_port(cid, port)?;
        Ok(Self { connection })
    }

    pub fn send<T: Serialize>(&self, msg: &T) -> Result<(), Error> {
        serde_json::to_writer(&self.connection, msg)?;
        Ok(())
    }

    pub fn recv<'de, T: Deserialize<'de>>(&self) -> Result<T, Error> {
        let mut de = Deserializer::from_reader(&self.connection);
        let res = T::deserialize(&mut de)?;

        Ok(res)
    }

    pub fn azero_account_id(&self) -> Result<AccountId32, Error> {
        self.send(&Command::AccountIdAzero)?;

        match self.recv()? {
            Response::AccountIdAzero { account_id } => Ok(account_id),
            other => Err(Error::InvalidResponse {
                expected: "AccountIdAzero".to_string(),
                got: other,
            }),
        }
    }

    pub fn sign_azero(&self, payload: &[u8]) -> Result<MultiSignature, Error> {
        self.send(&Command::SignAzero {
            payload: payload.to_vec(),
        })?;

        match self.recv()? {
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

    pub fn eth_address(&self) -> Result<EthAddress, Error> {
        self.send(&Command::EthAddress)?;

        match self.recv()? {
            Response::EthAddress { address } => Ok(address),
            other => Err(Error::InvalidResponse {
                expected: "EthAddress".to_string(),
                got: other,
            }),
        }
    }

    pub fn sign_eth_hash(&self, hash: EthH256) -> Result<EthSignature, Error> {
        self.send(&Command::SignEthHash { hash })?;

        match self.recv()? {
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

    pub fn sign_eth_tx(&self, tx: &EthTypedTransaction) -> Result<EthSignature, Error> {
        self.send(&Command::SignEthTx { tx: tx.clone() })?;

        match self.recv()? {
            Response::SignedEthTx {
                tx: return_tx,
                signature,
            } if return_tx == *tx => Ok(signature),
            other => Err(Error::InvalidResponse {
                expected: format!("SignedEthTx(tx: {:?})", tx),
                got: other,
            }),
        }
    }
}
