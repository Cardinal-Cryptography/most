use aleph_client::sp_runtime::print;
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
    InvalidResponse,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Ping,
    AccountIdAzero,
    Sign {
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
    Signed {
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

    pub fn account_id(&self) -> Result<AccountId32, Error> {
        self.send(&Command::AccountIdAzero)?;
        if let Response::AccountIdAzero { account_id } = self.recv()? {
            Ok(account_id)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub fn sign(&self, payload: &[u8]) -> Result<MultiSignature, Error> {
        self.send(&Command::Sign {
            payload: payload.to_vec(),
        })?;
        let signed = self.recv::<Response>()?;

        match signed {
            Response::Signed {
                payload: return_payload,
                signature,
            } if return_payload == payload => Ok(signature),
            _ => Err(Error::InvalidResponse),
        }
    }

    pub fn eth_address(&self) -> Result<EthAddress, Error> {
        self.send(&Command::EthAddress)?;

        if let Response::EthAddress { address } = self.recv()? {
            Ok(address)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub fn sign_eth_hash(&self, hash: EthH256) -> Result<EthSignature, Error> {
        self.send(&Command::SignEthHash { hash })?;

        if let Response::SignedEthHash {
            hash: return_hash,
            signature,
        } = self.recv()?
        {
            if return_hash == hash {
                return Ok(signature);
            }
        }

        Err(Error::InvalidResponse)
    }

    pub fn sign_eth_tx(&self, tx: &EthTypedTransaction) -> Result<EthSignature, Error> {
        self.send(&Command::SignEthTx { tx: tx.clone() })?;

        if let Response::SignedEthTx {
            tx: return_tx,
            signature,
        } = self.recv()?
        {
            if return_tx == *tx {
                return Ok(signature);
            }
        }

        Err(Error::InvalidResponse)
    }
}
