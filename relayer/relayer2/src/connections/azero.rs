use aleph_client::{
    sp_runtime::{MultiAddress, MultiSignature},
    AccountId, AlephConfig, AsConnection, Connection, KeyPair, Pair, RootConnection,
    SignedConnectionApi, TxInfo, TxStatus,
};
use anyhow::anyhow;
use log::debug;
use signer_client::Client;
use subxt::tx::TxPayload;
use tokio::sync::Mutex;

pub type AzeroWsConnection = Connection;
type ParamsBuilder = subxt::config::polkadot::PolkadotExtrinsicParamsBuilder<AlephConfig>;

pub async fn init(url: &str) -> AzeroWsConnection {
    Connection::new(url).await
}

struct AzeroSignerClient {
    client: Mutex<Client>,
    account_id: AccountId,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),

    #[error("Signer error: {0}")]
    SignerError(#[from] signer_client::Error),
}

impl AzeroSignerClient {
    async fn new(cid: u32, port: u32) -> Result<Self, Error> {
        let mut client = Client::new(cid, port).await?;
        let account_id = client.azero_account_id().await?;
        let client = Mutex::new(client);

        Ok(Self { client, account_id })
    }
}

enum AzeroSigner {
    Dev(Box<KeyPair>),
    Signer(AzeroSignerClient),
}

impl AzeroSigner {
    fn account_id(&self) -> &AccountId {
        match self {
            AzeroSigner::Dev(keypair) => keypair.account_id(),
            AzeroSigner::Signer(signer) => &signer.account_id,
        }
    }

    async fn sign(&self, payload: &[u8]) -> Result<MultiSignature, anyhow::Error> {
        match self {
            AzeroSigner::Dev(keypair) => Ok(keypair.signer().sign(payload).into()),
            AzeroSigner::Signer(signer) => {
                let mut client = signer.client.lock().await;
                let payload = payload.to_vec();
                let signature = client.sign_azero(&payload).await?;

                Ok(signature)
            }
        }
    }
}

pub struct AzeroConnectionWithSigner {
    connection: AzeroWsConnection,
    signer: AzeroSigner,
}

impl AzeroConnectionWithSigner {
    pub async fn with_signer(
        connection: AzeroWsConnection,
        cid: u32,
        port: u32,
    ) -> Result<Self, Error> {
        let client = AzeroSignerClient::new(cid, port).await?;
        let signer = AzeroSigner::Signer(client);
        Ok(Self { connection, signer })
    }

    pub fn with_keypair(connection: AzeroWsConnection, keypair: KeyPair) -> Self {
        let signer = AzeroSigner::Dev(Box::new(keypair));
        Self { connection, signer }
    }
}

impl AsConnection for AzeroConnectionWithSigner {
    fn as_connection(&self) -> &Connection {
        &self.connection
    }
}

#[async_trait::async_trait]
impl SignedConnectionApi for AzeroConnectionWithSigner {
    async fn send_tx<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        self.send_tx_with_params(tx, Default::default(), status)
            .await
    }

    async fn send_tx_with_params<Call: TxPayload + Send + Sync>(
        &self,
        tx: Call,
        params: ParamsBuilder,
        status: TxStatus,
    ) -> anyhow::Result<TxInfo> {
        if let Some(details) = tx.validation_details() {
            debug!(
                "Sending extrinsic {}.{} with params: {:?}",
                details.pallet_name, details.call_name, params
            );
        }

        let tx = self
            .as_connection()
            .as_client()
            .tx()
            .create_partial_signed(&tx, self.account_id(), params)
            .await?;
        let signature = self.signer.sign(&tx.signer_payload()).await?;
        let address = MultiAddress::Id(self.account_id().clone());
        let tx = tx.sign_with_address_and_signature(&address, &signature);

        let progress = tx
            .submit_and_watch()
            .await
            .map_err(|e| anyhow!("Failed to submit transaction: {:?}", e))?;

        let info: TxInfo = match status {
            TxStatus::InBlock => progress
                .wait_for_in_block()
                .await?
                .wait_for_success()
                .await?
                .into(),
            TxStatus::Finalized => progress.wait_for_finalized_success().await?.into(),
            // In case of Submitted block hash does not mean anything
            TxStatus::Submitted => {
                return Ok(TxInfo {
                    block_hash: Default::default(),
                    tx_hash: progress.extrinsic_hash(),
                })
            }
        };
        debug!(
            "tx with hash {:?} included in block {:?}",
            info.tx_hash, info.block_hash
        );

        Ok(info)
    }

    fn account_id(&self) -> &AccountId {
        self.signer.account_id()
    }

    fn signer(&self) -> &KeyPair {
        unimplemented!("AzeroConnectionWithSigner::signer")
    }

    async fn try_as_root(&self) -> anyhow::Result<RootConnection> {
        unimplemented!("AzeroConnectionWithSigner::try_as_root")
    }
}
