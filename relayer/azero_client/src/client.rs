use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use log::trace;
use pallet_contracts_primitives::ContractExecResult;
use parity_scale_codec::Decode;
use subxt::{
    backend::{
        legacy::LegacyRpcMethods,
        rpc::reconnecting_rpc_client::{Client as RpcClient, ExponentialBackoff},
    },
    config::DefaultExtrinsicParamsBuilder,
    dynamic::Value,
    error::RpcError,
    ext::scale_value::value,
    runtime_api::RuntimeApi,
    tx::{PartialExtrinsic, Payload, SubmittableExtrinsic},
    utils::MultiAddress,
    Error, OnlineClient, PolkadotConfig,
};

use crate::{
    translate_events, AccountId, Balance, BlockHash, ContractCallArgs, ContractEvent,
    ContractInstance, EventRecord, MultiSignature, Signer, TxInfo, Weight,
};

const LOG_TARGET: &str = "AzeroClient";

fn get_args_for_runtime_call(args: ContractCallArgs) -> Vec<Value> {
    let gas_limit = match args.gas_limit {
        Some(w) => Value::unnamed_variant(
            "Some",
            vec![Value::named_composite(vec![
                ("ref_time", Value::u128(w.ref_time as u128)),
                ("proof_size", Value::u128(w.proof_size as u128)),
            ])],
        ),
        None => Value::unnamed_variant("None", vec![]),
    };
    vec![
        Value::from_bytes(args.origin),
        Value::from_bytes(args.dest),
        Value::u128(args.value),
        gas_limit,
        Value::unnamed_variant("None", vec![]),
        Value::from_bytes(args.input_data),
    ]
}

fn get_args_for_rpc_call(
    weight: Weight,
    contract_address: AccountId,
    value: Balance,
    call_data: Vec<u8>,
) -> Vec<Value> {
    let gas_limit: Value = value! {
        { ref_time : weight.ref_time, proof_size : weight.proof_size }
    };
    let dest = value! { Id( Value::from_bytes(contract_address)) };

    vec![
        dest,
        Value::u128(value),
        gas_limit,
        Value::unnamed_variant("None", vec![]),
        Value::from_bytes(call_data),
    ]
}

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("RpcError: {0}")]
    Rpc(#[from] RpcError),
    #[error("SubxtError: {0}")]
    Subxt(#[from] Error),
    #[error("No block with number: {0}")]
    NoBlockWithNumber(u32),
    #[error("InternalError")]
    Internal,
}

pub type ClientResult<T> = Result<T, ClientError>;

pub struct ClientConfig {
    pub address: String,
    pub backoff_millis: u64,
    pub backoff_factor: u64,
    pub backoff_max_delay: Duration,
}

impl ClientConfig {
    fn exponential_backoff(
        &self,
    ) -> impl Iterator<Item = Duration> + Send + Sync + 'static + Clone {
        ExponentialBackoff::from_millis(self.backoff_millis)
            .factor(self.backoff_factor)
            .max_delay(self.backoff_max_delay)
            .take(10)
    }

    fn ws_address(&self) -> String {
        self.address.clone()
    }
}

#[derive(Clone)]
pub struct Client {
    inner: OnlineClient<PolkadotConfig>,
    methods: LegacyRpcMethods<PolkadotConfig>,
}

impl Client {
    pub async fn new(config: &ClientConfig) -> ClientResult<Self> {
        let rpc = RpcClient::builder()
            .retry_policy(config.exponential_backoff())
            .build(config.ws_address())
            .await?;
        let methods = LegacyRpcMethods::new(subxt::backend::rpc::RpcClient::new(rpc.clone()));
        let inner = OnlineClient::from_rpc_client(rpc).await?;

        Ok(Self { inner, methods })
    }

    pub async fn get_finalized_block_hash(&self) -> ClientResult<BlockHash> {
        Ok(self.methods.chain_get_finalized_head().await?)
    }

    pub async fn get_block_number(&self, block_hash: BlockHash) -> ClientResult<Option<u32>> {
        Ok(self
            .methods
            .chain_get_block(Some(block_hash))
            .await?
            .map(|d| d.block.header.number))
    }

    pub async fn with_signer<S: Signer>(&self, signer: S) -> ClientResult<ClientWithSigner<S>> {
        ClientWithSigner::new(self.clone(), signer).await
    }

    async fn get_runtime_api_at(
        &self,
        at: Option<BlockHash>,
    ) -> ClientResult<RuntimeApi<PolkadotConfig, OnlineClient<PolkadotConfig>>> {
        Ok(match at {
            Some(at) => self.inner.runtime_api().at(at),
            _ => self.inner.runtime_api().at_latest().await?,
        })
    }

    pub async fn contract_call_and_get(
        &self,
        args: ContractCallArgs,
        at: Option<BlockHash>,
    ) -> ClientResult<ContractExecResult<Balance, EventRecord>> {
        let args_data = get_args_for_runtime_call(args);
        let payload = subxt::runtime_api::dynamic("ContractsApi", "call", args_data);

        let runtime_api = self.get_runtime_api_at(at).await?;

        ContractExecResult::decode(&mut runtime_api.call(payload).await?.encoded())
            .map_err(|_| ClientError::Internal)
    }

    pub async fn fetch_events_from_contracts(
        &self,
        at_block: u32,
        contracts: &[&ContractInstance],
    ) -> ClientResult<Vec<ContractEvent>> {
        let block_hash = self
            .methods
            .chain_get_block_hash(Some(at_block.into()))
            .await?
            .ok_or(ClientError::NoBlockWithNumber(at_block))?;

        let events = self.inner.blocks().at(block_hash).await?.events().await?;

        let events_from_block = translate_events(events.iter(), contracts)
            .into_iter()
            .filter_map(|e| match e {
                Ok(event) => Some(event),
                Err(error) => {
                    trace!(target: LOG_TARGET, "Decode event failed, {:?}", error);
                    None
                }
            })
            .collect();

        Ok(events_from_block)
    }
}

pub struct ClientWithSigner<S: Signer> {
    client: Client,
    signer: S,
    nonce: AtomicU64,
}

impl<S: Signer> ClientWithSigner<S> {
    pub async fn new(client: Client, signer: S) -> ClientResult<Self> {
        let nonce = client.inner.tx().account_nonce(signer.account_id()).await?;

        Ok(Self {
            client,
            signer,
            nonce: AtomicU64::new(nonce),
        })
    }
    fn get_tx<Call: Payload>(
        &self,
        call: &Call,
    ) -> ClientResult<PartialExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>> {
        let nonce = self.nonce.load(Ordering::Relaxed);
        let params = DefaultExtrinsicParamsBuilder::default()
            .nonce(nonce)
            .build();

        let tx = self
            .client
            .inner
            .tx()
            .create_partial_signed_offline(call, params)?;

        Ok(tx)
    }

    async fn sign_call<Call: Payload>(&self, call: &Call) -> ClientResult<MultiSignature> {
        // PartialExtrinsic is not Send when Call is of type DynamicPayload,
        // so it cant live past any await.
        let payload = {
            let tx = self.get_tx(call)?;
            tx.signer_payload()
        };

        let signature = self
            .signer
            .sign(&payload)
            .await
            .map_err(|_| ClientError::Internal)?;

        Ok(signature)
    }

    async fn get_submittable<Call: Payload + Send + Sync>(
        &self,
        call: Call,
    ) -> ClientResult<SubmittableExtrinsic<PolkadotConfig, OnlineClient<PolkadotConfig>>> {
        let signature = self.sign_call(&call).await?;
        let address = MultiAddress::Id(self.signer.account_id().clone());

        let extr = self.get_tx(&call)?;

        Ok(extr.sign_with_address_and_signature(&address, &signature))
    }

    async fn send_tx_with_params<Call: Payload + Send + Sync>(
        &self,
        tx: Call,
    ) -> ClientResult<TxInfo> {
        let tx = self.get_submittable(tx).await?;
        self.inc_nonce();

        let events = tx
            .submit_and_watch()
            .await?
            .wait_for_finalized_success()
            .await?;

        Ok(events.into())
    }

    pub async fn contract_call(
        &self,
        contract_address: AccountId,
        value: Balance,
        weight: Weight,
        call_data: Vec<u8>,
    ) -> ClientResult<TxInfo> {
        let args = get_args_for_rpc_call(weight, contract_address, value, call_data);

        let payload = subxt::tx::dynamic("Contracts", "call", args);

        self.send_tx_with_params(payload).await
    }

    fn inc_nonce(&self) -> u64 {
        self.nonce.fetch_add(1, Ordering::Relaxed)
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn account_id(&self) -> &AccountId {
        self.signer.account_id()
    }
}
