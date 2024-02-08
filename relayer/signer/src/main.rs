use std::thread;

use clap::Parser;
use log::info;
use signer_client::{Client, Command, Response};
use subxt::ext::{
    sp_core::{crypto::SecretStringError, sr25519::Pair as KeyPair, Pair},
    sp_runtime::AccountId32,
};
use vsock::{VsockAddr, VsockListener, VMADDR_CID_ANY};

#[derive(Parser)]
struct ServerArguments {
    #[clap(short, long, default_value = "1234")]
    port: u32,

    #[clap(short, long)]
    azero_key: Option<String>,
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
    env_logger::init();

    let args = ServerArguments::parse();
    let server = Server::new(
        args.azero_key.expect("Aleph Zero key not provided"),
        args.port,
    )?;

    info!("Server listening on: {:?}", server.local_addr()?);
    info!("Azero account ID: {:?}", server.account_id());

    server.accept_loop()?;

    Ok(())
}

struct Server {
    listener: VsockListener,
    azero_key: KeyPair,
}

impl Server {
    fn new(azero_key: String, port: u32) -> Result<Self, Error> {
        let azero_key = KeyPair::from_string(&azero_key, None)?;
        let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, port)?;

        Ok(Self {
            listener,
            azero_key,
        })
    }

    fn account_id(&self) -> AccountId32 {
        self.azero_key.public().into()
    }

    fn local_addr(&self) -> Result<VsockAddr, Error> {
        Ok(self.listener.local_addr()?)
    }

    fn accept_one(&self) -> Result<(), Error> {
        let (client, _) = self.listener.accept()?;
        let client = Client::from(client);
        handle_client(client, self.azero_key.clone());

        Ok(())
    }

    fn accept_loop(&self) -> Result<(), Error> {
        loop {
            self.accept_one()?;
        }
    }
}

fn handle_client(client: Client, azero_key: KeyPair) {
    thread::spawn(move || {
        let result = do_handle_client(client, &azero_key);
        info!("Client disconnected: {:?}", result);
    });
}

fn do_handle_client(client: Client, azero_key: &KeyPair) -> Result<(), Error> {
    loop {
        let command = client.recv()?;
        info!("Received command: {:?}", command);

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

#[cfg(test)]
mod test {
    use std::env;

    use assert2::{assert, let_assert};
    use serial_test::serial;
    use subxt::ext::sp_runtime::traits::Verify;
    use vsock::VMADDR_CID_HOST;

    use super::*;

    #[test]
    #[serial]
    fn test_ping() {
        let client = connect();

        client.send(&Command::Ping).unwrap();
        let response: Response = client.recv().unwrap();

        assert!(matches!(response, Response::Pong));
    }

    #[test]
    #[serial]
    fn test_account_id() {
        let client = connect();

        client.send(&Command::AccountId).unwrap();
        let response: Response = client.recv().unwrap();

        let_assert!(Response::AccountId { account_id } = response);
        assert!(account_id.to_string() == "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY");
    }

    #[test]
    #[serial]
    fn test_sign() {
        let client = connect();
        let payload = b"Hello, world!".to_vec();

        client
            .send(&Command::Sign {
                payload: payload.clone(),
            })
            .unwrap();
        let response: Response = client.recv().unwrap();

        let_assert!(
            Response::Signed {
                payload: signed_payload,
                signature
            } = response
        );

        assert!(signed_payload == payload);
        assert!(signature.verify(&payload[..], &client.account_id().unwrap()));
    }

    fn connect() -> Client {
        let server = Server::new("//Alice".to_string(), port()).unwrap();
        let client = Client::new(VMADDR_CID_HOST, port()).unwrap();
        server.accept_one().unwrap();

        client
    }

    fn port() -> u32 {
        env::var("PORT")
            .unwrap_or_else(|_| "9876".to_string())
            .parse()
            .unwrap()
    }
}
