use clap::Parser;
use signer_client::{client, Client, Command, Response};
use subxt::ext::sp_core::{crypto::SecretStringError, sr25519::Pair as KeyPair, Pair};
use vsock::{VsockListener, VMADDR_CID_ANY, VMADDR_CID_HOST};

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
    Stream(signer_client::Error),

    #[error("Key error: {0}")]
    Key(SecretStringError),
}

impl From<signer_client::Error> for Error {
    fn from(err: signer_client::Error) -> Self {
        Error::Stream(err)
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

impl From<SecretStringError> for Error {
    fn from(value: SecretStringError) -> Self {
        Error::Key(value)
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
    let azero_key = KeyPair::from_string(&azero_key, None)?;

    let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, 1234)?;

    for client in listener.incoming() {
        let client: Client = client?.into();
        let result = handle_client(client, &azero_key);
        println!("Client disconnected: {:?}", result);
    }

    Ok(())
}

fn handle_client(client: Client, azero_key: &KeyPair) -> Result<(), Error> {
    loop {
        let command = client.recv()?;
        println!("Received command: {:?}", command);

        match command {
            Command::Ping => {
                client.send(&Response::Pong)?;
            }
            Command::Sign { payload } => {
                let signature = azero_key.sign(&payload);
                let signature = subxt::ext::sp_runtime::MultiSignature::Sr25519(signature);
                client.send(&Response::Signed { payload, signature })?;
            }
        }
    }
}
