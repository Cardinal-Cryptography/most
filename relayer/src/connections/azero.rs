use std::sync::Arc;

use aleph_client::Connection;

pub struct AzeroConnection;

pub type AzeroWsConnection = Arc<Connection>;

impl AzeroConnection {
    pub async fn init(url: &str) -> AzeroWsConnection {
        Arc::new(Connection::new(url).await)
    }
}
