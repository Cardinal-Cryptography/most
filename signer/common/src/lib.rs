use schnorrkel::SIGNATURE_LENGTH;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use serde_json::Deserializer;
use vsock::VsockStream;

pub type SerializedSignature = [u8; SIGNATURE_LENGTH];

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(std::io::Error),
    #[error("Serde error: {0}")]
    Serde(serde_json::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::IO(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serde(err)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Command {
    Ping,
    Sign { payload: Vec<u8> },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Pong,
    Signed {
        payload: Vec<u8>,
        #[serde(with = "BigArray")]
        signature: SerializedSignature,
    },
}

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
}

pub fn client(cid: u32, port: u32) -> Result<(), Error> {
    let client = Client::new(cid, port)?;

    client.send(&Command::Ping)?;
    let res: Response = client.recv()?;
    println!("Received response: {:?}", res);

    client.send(&Command::Sign {
        payload: vec![1, 2, 3, 4],
    })?;
    let res: Response = client.recv()?;
    println!("Received response: {:?}", res);

    Ok(())
}
