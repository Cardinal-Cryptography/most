use clap::Parser;
use common::{client, Command, Response};
use hex::FromHexError;
use schnorrkel::{signing_context, Keypair, SignatureError};
use serde::Deserialize;
use vsock::{VsockListener, VsockStream, VMADDR_CID_ANY, VMADDR_CID_HOST};

#[derive(Parser)]
struct ServerArguments {
    #[clap(short, long, default_value = "1234")]
    port: u16,

    #[clap(short, long)]
    azero_key: Option<String>,

    #[clap(short, long)]
    server: bool,
}

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("Stream error: {0}")]
    Stream(common::Error),

    #[error("Key error: {0}")]
    Key(String),

    #[error("Signature error: {0}")]
    Signature(SignatureError),
}

impl From<common::Error> for Error {
    fn from(err: common::Error) -> Self {
        Error::Stream(err)
    }
}

impl From<FromHexError> for Error {
    fn from(err: FromHexError) -> Self {
        Error::Key("The key should be hex-encoded".to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Stream(err.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Stream(err.into())
    }
}

impl From<SignatureError> for Error {
    fn from(err: SignatureError) -> Self {
        Error::Signature(err)
    }
}

fn main() -> Result<(), Error> {
    let args = ServerArguments::parse();

    if args.server {
        server(args.azero_key.expect("Aleph Zero key not provided"))?
    } else {
        client(VMADDR_CID_HOST, 1234)?
    }

    Ok(())
}

fn server(azero_key: String) -> Result<(), Error> {
    let azero_key = hex::decode(&azero_key)?;
    let azero_key = Keypair::from_half_ed25519_bytes(&azero_key[..])?;

    let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, 1234)?;

    for client in listener.incoming() {
        let client = client?;
        println!("Receive connection from: {:?}", client.peer_addr());

        let result = handle_client(client, &azero_key);
        println!("Client disconnected: {:?}", result);
    }

    Ok(())
}

fn handle_client(client: VsockStream, azero_key: &Keypair) -> Result<(), Error> {
    let mut de = serde_json::Deserializer::from_reader(&client);

    loop {
        let command = Command::deserialize(&mut de)?;
        println!("Received command: {:?}", command);

        match command {
            Command::Ping => {
                serde_json::to_writer(&client, &Response::Pong)?;
            }
            Command::Sign { payload } => {
                let context = signing_context("MOST signer".as_bytes());
                let signature = azero_key.sign(context.bytes(&payload));
                let signature = signature.to_bytes();

                serde_json::to_writer(&client, &Response::Signed { payload, signature })?;
            }
        }
    }
}
