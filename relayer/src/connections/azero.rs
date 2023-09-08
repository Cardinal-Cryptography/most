use std::sync::Arc;

use aleph_client::{Connection, KeyPair, SignedConnection};

pub type AzeroWsConnection = Arc<Connection>;

pub async fn init(url: &str) -> AzeroWsConnection {
    Arc::new(Connection::new(url).await)
}

pub fn sign(connection: AzeroWsConnection, keypair: &KeyPair) -> SignedConnection {
    let signer = KeyPair::new(keypair.signer().clone());
    let connection = Connection {
        client: connection.client.clone(),
    };
    SignedConnection { connection, signer }
}
