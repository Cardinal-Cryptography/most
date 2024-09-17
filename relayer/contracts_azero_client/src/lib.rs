mod client;
mod contract;
mod keypair;
mod types;

use std::fmt::Debug;

pub use client::{Client, ClientConfig, ClientError, ClientWithSigner};
pub use contract::*;
pub use contract_transcode;
pub use keypair::*;
pub use types::*;

#[async_trait::async_trait]
pub trait Signer: Send + Sync + 'static {
    type Error: Debug;

    fn account_id(&self) -> &AccountId;
    async fn sign(&self, payload: &[u8]) -> Result<MultiSignature, Self::Error>;
}
