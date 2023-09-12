use aleph_client::{Connection, KeyPair, SignedConnection};

pub type AzeroWsConnection = Connection;
pub type SignedAzeroWsConnection = SignedConnection;

pub async fn init(url: &str) -> AzeroWsConnection {
    Connection::new(url).await
}

pub fn sign(connection: &AzeroWsConnection, keypair: &KeyPair) -> SignedAzeroWsConnection {
    let signer = KeyPair::new(keypair.signer().clone());
    let connection = Connection {
        client: connection.client.clone(),
    };
    SignedAzeroWsConnection { connection, signer }
}
