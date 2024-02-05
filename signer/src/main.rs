use std::env;

use serde::{Deserialize, Serialize};
use vsock::{VsockListener, VsockStream, VMADDR_CID_ANY, VMADDR_CID_HOST};

#[derive(thiserror::Error, Debug)]
enum Error {
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
enum Command {
    Ping,
}

#[derive(Serialize, Deserialize, Debug)]
enum Response {
    Pong,
}

fn main() -> Result<(), Error> {
    let args = env::args().collect::<Vec<_>>();

    println!("args: {:?}", args);

    if args.len() == 2 && args[1] == "client" {
        client()
    } else {
        server()
    }
}

fn client() -> Result<(), Error> {
    let connection = VsockStream::connect_with_cid_port(VMADDR_CID_HOST, 1234)?;
    let mut de = serde_json::Deserializer::from_reader(&connection);

    serde_json::to_writer(&connection, &Command::Ping)?;
    let res = Response::deserialize(&mut de)?;
    println!("Received response: {:?}", res);

    Ok(())
}

fn server() -> Result<(), Error> {
    let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, 1234)?;

    for client in listener.incoming() {
        let client = client?;
        println!("Receive connection from: {:?}", client.peer_addr()?);

        let result = handle_client(client);
        println!("Client disconnected: {:?}", result);
    }

    Ok(())
}

fn handle_client(client: VsockStream) -> Result<(), Error> {
    let mut de = serde_json::Deserializer::from_reader(&client);

    loop {
        let command = Command::deserialize(&mut de)?;
        println!("Received command: {:?}", command);

        match command {
            Command::Ping => {
                serde_json::to_writer(&client, &Response::Pong)?;
            }
        }
    }
}
