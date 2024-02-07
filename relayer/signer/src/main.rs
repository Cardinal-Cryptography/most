use std::thread;

use clap::Parser;
use signer_client::{client, Client, Command, Response};
use subxt::ext::{
    sp_core::{crypto::SecretStringError, sr25519::Pair as KeyPair, Pair},
    sp_runtime::AccountId32,
};
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
    Stream(#[from] signer_client::Error),

    #[error("Key error: {0}")]
    Key(#[from] SecretStringError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
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
    let account_id: AccountId32 = azero_key.public().into();
    println!("Account ID: {:?}", account_id);

    let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, 1234)?;
    println!("Vsock address: {:?}", listener.local_addr());

    for client in listener.incoming() {
        let client: Client = client?.into();
        handle_client(client, azero_key.clone());
    }

    Ok(())
}

fn handle_client(client: Client, azero_key: KeyPair) {
    thread::spawn(move || {
        let result = do_handle_client(client, &azero_key);
        println!("Client disconnected: {:?}", result);
    });
}

fn do_handle_client(client: Client, azero_key: &KeyPair) -> Result<(), Error> {
    loop {
        let command = client.recv()?;
        println!("Received command: {:?}", command);

        match command {
            Command::Ping => {
                client.send(&Response::Pong)?;
            }

            Command::AccountId => {
                let account_id = azero_key.public().into();
                client.send(&Response::AccountId { account_id })?;
            }

            Command::Sign { payload } => {
                let signature = azero_key.sign(&payload);
                let signature = subxt::ext::sp_runtime::MultiSignature::Sr25519(signature);

                client.send(&Response::Signed { payload, signature })?;
            }
        }
    }
}
