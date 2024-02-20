use std::thread;

use clap::Parser;
use ethers::{
    signers::{LocalWallet, Signer},
    types::Address,
};
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
    azero_key: String,

    #[clap(short, long)]
    eth_key: String,
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

    #[error("Eth Wallet error: {0}")]
    Wallet(#[from] ethers::signers::WalletError),

    #[error("Hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),
}

fn main() -> Result<(), Error> {
    env_logger::init();

    let args = ServerArguments::parse();
    let server = Server::new(args.azero_key, args.eth_key, args.port)?;

    info!("Server listening on: {:?}", server.local_addr()?);
    info!("Azero account ID: {:?}", server.azero_account_id());
    info!("ETH address: {:?}", server.eth_address());

    server.accept_loop()?;

    Ok(())
}

struct Server {
    listener: VsockListener,
    azero_key: KeyPair,
    eth_wallet: LocalWallet,
}

impl Server {
    fn new(azero_key: String, eth_key: String, port: u32) -> Result<Self, Error> {
        let azero_key = KeyPair::from_string(&azero_key, None)?;
        let listener = VsockListener::bind_with_cid_port(VMADDR_CID_ANY, port)?;
        let eth_key = hex::decode(eth_key)?;
        let eth_wallet = LocalWallet::from_bytes(&eth_key)?;

        Ok(Self {
            listener,
            azero_key,
            eth_wallet,
        })
    }

    fn azero_account_id(&self) -> AccountId32 {
        self.azero_key.public().into()
    }

    fn eth_address(&self) -> Address {
        self.eth_wallet.address()
    }

    fn local_addr(&self) -> Result<VsockAddr, Error> {
        Ok(self.listener.local_addr()?)
    }

    fn accept_one(&self) -> Result<(), Error> {
        let (client, _) = self.listener.accept()?;
        let client = Client::from(client);
        handle_client(client, self.azero_key.clone(), self.eth_wallet.clone());

        Ok(())
    }

    fn accept_loop(&self) -> Result<(), Error> {
        loop {
            self.accept_one()?;
        }
    }
}

fn handle_client(client: Client, azero_key: KeyPair, eth_wallet: LocalWallet) {
    thread::spawn(move || {
        let result = do_handle_client(client, &azero_key, &eth_wallet);
        info!("Client disconnected: {:?}", result);
    });
}

fn do_handle_client(
    client: Client,
    azero_key: &KeyPair,
    eth_wallet: &LocalWallet,
) -> Result<(), Error> {
    loop {
        let command = client.recv()?;
        info!("Received command: {:?}", command);

        match command {
            Command::Ping => {
                client.send(&Response::Pong)?;
            }

            Command::AccountIdAzero => {
                let account_id = azero_key.public().into();
                client.send(&Response::AccountIdAzero { account_id })?;
            }

            Command::SignAzero { payload } => {
                let signature = azero_key.sign(&payload);
                let signature = subxt::ext::sp_runtime::MultiSignature::Sr25519(signature);

                client.send(&Response::SignedAzero { payload, signature })?;
            }

            Command::EthAddress => {
                let address = eth_wallet.address();
                client.send(&Response::EthAddress { address })?;
            }

            Command::SignEthHash { hash } => {
                let signature = eth_wallet.sign_hash(hash)?;
                client.send(&Response::SignedEthHash { hash, signature })?;
            }

            Command::SignEthTx { mut tx, chain_id } => {
                tx.set_chain_id(chain_id);
                let signature = eth_wallet.sign_transaction_sync(&tx)?;
                client.send(&Response::SignedEthTx {
                    tx,
                    chain_id,
                    signature,
                })?;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::{env, str::FromStr};

    use assert2::{assert, let_assert};
    use ethers::{addressbook::Address, types::transaction::eip2718::TypedTransaction};
    use serial_test::serial;
    use subxt::ext::sp_runtime::traits::Verify;
    use vsock::VMADDR_CID_HOST;

    use super::*;

    const ETH_PUBLIC_ADDRESS: &str = "0xEe88da44b4901d7F86970c52dC5139Af80C83edD";
    const ETH_PRIVATE_KEY: &str =
        "58039a48427a62f77e5562d7f565d10595d92abdd4813233607ec2ac5ac4b9de";
    const ETH_MAINNET_CHAIN_ID: u64 = 1;

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
    fn test_account_id_azero() {
        let client = connect();

        client.send(&Command::AccountIdAzero).unwrap();
        let response: Response = client.recv().unwrap();

        let_assert!(Response::AccountIdAzero { account_id } = response);
        assert!(account_id.to_string() == "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY");
    }

    #[test]
    #[serial]
    fn test_sign_azero() {
        let client = connect();
        let payload = b"Hello, world!".to_vec();

        client
            .send(&Command::SignAzero {
                payload: payload.clone(),
            })
            .unwrap();
        let response: Response = client.recv().unwrap();

        let_assert!(
            Response::SignedAzero {
                payload: signed_payload,
                signature
            } = response
        );

        assert!(signed_payload == payload);
        assert!(signature.verify(&payload[..], &client.azero_account_id().unwrap()));
    }

    #[test]
    #[serial]
    fn test_eth_address() {
        let client = connect();

        let address = client.eth_address().unwrap();

        assert!(address == Address::from_str(ETH_PUBLIC_ADDRESS).unwrap());
    }

    #[test]
    #[serial]
    fn test_sign_eth_hash() {
        let client = connect();
        let payload = b"Hello, world!".to_vec();
        let hash = ethers::utils::keccak256(payload).into();

        let signature = client.sign_eth_hash(hash).unwrap();

        let address = Address::from_str(ETH_PUBLIC_ADDRESS).unwrap();
        assert!(signature.verify(hash, address).is_ok());
    }

    #[test]
    #[serial]
    fn test_sign_eth_tx_without_chain_id() {
        let client = connect();
        let mut tx = TypedTransaction::Eip1559(Default::default());

        let signature = client.sign_eth_tx(&tx).unwrap();

        // Transactions with no chain id set should be treated as mainnet transactions
        tx.set_chain_id(ETH_MAINNET_CHAIN_ID);
        let address = Address::from_str(ETH_PUBLIC_ADDRESS).unwrap();
        let hash = tx.sighash();
        assert!(signature.verify(hash, address).is_ok())
    }

    #[test]
    #[serial]
    fn test_sign_eth_tx_with_chain_id() {
        let client = connect();
        let mut tx = TypedTransaction::Eip1559(Default::default());
        tx.set_chain_id(1337);

        let signature = client.sign_eth_tx(&tx).unwrap();

        let address = Address::from_str(ETH_PUBLIC_ADDRESS).unwrap();
        let hash = tx.sighash();
        assert!(signature.verify(hash, address).is_ok())
    }

    fn connect() -> Client {
        let server =
            Server::new("//Alice".to_string(), ETH_PRIVATE_KEY.to_string(), port()).unwrap();
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
