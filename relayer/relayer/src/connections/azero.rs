use aleph_client::{
    AccountId, AlephConfig, AsConnection, Connection, KeyPair, RootConnection, SignedConnection,
    SignedConnectionApi, TxInfo, TxStatus,
};
use anyhow::anyhow;
use log::info;
use signer_client::Client;
use subxt::tx::TxPayload;

pub type AzeroWsConnection = Connection;
pub type SignedAzeroWsConnection = SignedConnection;
type ParamsBuilder = subxt::config::polkadot::PolkadotExtrinsicParamsBuilder<AlephConfig>;

pub async fn init(url: &str) -> AzeroWsConnection {
    Connection::new(url).await
}

pub fn sign(connection: &AzeroWsConnection, keypair: &KeyPair) -> SignedAzeroWsConnection {
    let signer = KeyPair::new(keypair.signer().clone());
    SignedAzeroWsConnection::from_connection(connection.clone(), signer)
}

pub struct AzeroConnectionWithSigner {
    connection: AzeroWsConnection,
    signer: Client,
    account_id: AccountId,
}

impl AzeroConnectionWithSigner {
    pub fn new(
        connection: AzeroWsConnection,
        cid: u32,
        port: u32,
    ) -> Result<Self, signer_client::Error> {
        let signer = Client::new(cid, port)?;
        let account_id = signer.account_id()?;
        Ok(Self {
            connection,
            signer,
            account_id,
        })
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
            info!(target:"aleph-client", "Sending extrinsic {}.{} with params: {:?}", details.pallet_name, details.call_name, params);
        }

        let tx = self
            .as_connection()
            .as_client()
            .tx()
            .create_partial_signed(&tx, &self.account_id, params)
            .await?;
        let signer = self.signer.prepare_signer(&tx.signer_payload())?;
        let tx = tx.sign(&signer);

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
        info!(target: "aleph-client", "tx with hash {:?} included in block {:?}", info.tx_hash, info.block_hash);

        Ok(info)
    }

    fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    fn signer(&self) -> &KeyPair {
        unimplemented!("AzeroConnectionWithSigner::signer")
    }

    async fn try_as_root(&self) -> anyhow::Result<RootConnection> {
        unimplemented!("AzeroConnectionWithSigner::try_as_root")
    }
}
