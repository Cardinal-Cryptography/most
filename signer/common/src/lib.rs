use schnorrkel::SIGNATURE_LENGTH;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
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

pub fn client(cid: u32, port: u32) -> Result<(), Error> {
    let connection = VsockStream::connect_with_cid_port(cid, port)?;
    let mut de = serde_json::Deserializer::from_reader(&connection);

    serde_json::to_writer(&connection, &Command::Ping)?;
    let res = Response::deserialize(&mut de)?;
    println!("Received response: {:?}", res);

    serde_json::to_writer(
        &connection,
        &Command::Sign {
            payload: vec![1, 2, 3, 4],
        },
    )?;
    let res = Response::deserialize(&mut de)?;
    println!("Received response: {:?}", res);

    Ok(())
}
