use std::sync::Arc;

use aleph_client::contract::event::ContractEvent;
use ethers::{
    abi::{self, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    types::U64,
    utils::keccak256,
};
use log::{debug, error, info, warn};
use subxt::utils::H256;
use thiserror::Error;
use tokio::time::{sleep, Duration};

use crate::{
    config::Config,
    connections::eth::SignedEthConnection,
    contracts::{get_request_event_data, AzeroContractError, CrosschainTransferRequestData, Most},
    listeners::{get_next_finalized_block_number_eth, ETH_BLOCK_PROD_TIME_SEC},
};

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AlephZeroHandlerError {
    #[error("Error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("Ethers provider error")]
    Provider(#[from] ProviderError),

    #[error("Azero contract error")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Eth contract error")]
    EthContractTx(#[from] ContractError<SignedEthConnection>),

    #[error("Tx was not present in any block or mempool after the maximum number of retries")]
    TxNotPresentInBlockOrMempool,

    #[error("Contract reverted")]
    EthContractReverted,
}

pub struct AlephZeroHandler;

impl AlephZeroHandler {
    pub async fn handle_event(
        config: Arc<Config>,
        eth_connection: Arc<SignedEthConnection>,
        event: ContractEvent,
    ) -> Result<(), AlephZeroHandlerError> {
        let Config {
            eth_contract_address,
            eth_tx_min_confirmations,
            eth_tx_submission_retries,
            ..
        } = &*config;

        if let Some(name) = &event.name {
            if name.eq("CrosschainTransferRequest") {
                let data = event.data;

                // decode event data
                let CrosschainTransferRequestData {
                    committee_id,
                    dest_token_address,
                    amount,
                    dest_receiver_address,
                    request_nonce,
                } = get_request_event_data(&data)?;

                info!(
                "Decoded event data: [dest_token_address: 0x{}, amount: {amount}, dest_receiver_address: 0x{}, request_nonce: {request_nonce}]",
                hex::encode(dest_token_address),
                hex::encode(dest_receiver_address)
            );

                // NOTE: for some reason, ethers-rs's `encode_packed` does not properly encode the data
                // (it does not pad uint to 32 bytes, but uses the actual number of bytes required to store the value)
                // so we use `abi::encode` instead (it only differs for signed and dynamic size types, which we don't use here)
                let bytes = abi::encode(&[
                    Token::Uint(committee_id.into()),
                    Token::FixedBytes(dest_token_address.to_vec()),
                    Token::Uint(amount.into()),
                    Token::FixedBytes(dest_receiver_address.to_vec()),
                    Token::Uint(request_nonce.into()),
                ]);

                debug!("ABI event encoding: 0x{}", hex::encode(bytes.clone()));

                let request_hash = keccak256(bytes);

                info!("hashed event encoding: 0x{}", hex::encode(request_hash));

                let address = eth_contract_address.parse::<Address>()?;
                let contract = Most::new(address, eth_connection.clone());

                // forward transfer & vote
                let call: ContractCall<SignedEthConnection, ()> = contract.receive_request(
                    request_hash,
                    committee_id.into(),
                    dest_token_address,
                    amount.into(),
                    dest_receiver_address,
                    request_nonce.into(),
                );

                info!(
                "Sending tx with request nonce {} to the Ethereum network and waiting for {} confirmations.",
                request_nonce,
                eth_tx_min_confirmations
            );

                // This shouldn't fail unless there is something wrong with our config.
                // NOTE: this does not check whether the actual tx reverted on-chain. Reverts are only checked on dry-run.
                let receipt = call
                    .gas(config.eth_gas_limit)
                    .nonce(eth_connection.inner().next())
                    .send()
                    .await?
                    .confirmations(*eth_tx_min_confirmations)
                    .retries(*eth_tx_submission_retries)
                    .await?
                    .ok_or(AlephZeroHandlerError::TxNotPresentInBlockOrMempool)?;

                let tx_hash = receipt.transaction_hash;
                let tx_status = receipt.status;

                // Check if the tx reverted.
                if tx_status == Some(U64::from(0)) {
                    warn!(
                    "Tx with nonce {request_nonce} has been sent to the Ethereum network: {tx_hash:?} but it reverted."
                );
                    return Err(AlephZeroHandlerError::EthContractReverted);
                }

                info!("Tx with nonce {request_nonce} has been sent to the Ethereum network: {tx_hash:?} and received {eth_tx_min_confirmations} confirmations.");

                wait_for_eth_tx_finality(eth_connection, tx_hash).await?;
            }
        }
        Ok(())
    }
}

async fn wait_for_eth_tx_finality(
    eth_connection: Arc<SignedEthConnection>,
    tx_hash: H256,
) -> Result<(), AlephZeroHandlerError> {
    info!("Waiting for tx finality: {tx_hash:?}");
    loop {
        sleep(Duration::from_secs(ETH_BLOCK_PROD_TIME_SEC)).await;

        let connection_rc = Arc::new(eth_connection.provider().clone());
        let finalized_head_number = get_next_finalized_block_number_eth(connection_rc, 0).await;

        match eth_connection.get_transaction(tx_hash).await {
            Ok(Some(tx)) => {
                if let Some(block_number) = tx.block_number {
                    if block_number <= finalized_head_number.into() {
                        info!("Eth tx {tx_hash:?} finalized");
                        return Ok(());
                    }
                }
            }
            Err(err) => {
                error!("Failed to get tx that should be present: {err}");
            }
            Ok(None) => panic!("Transaction {tx_hash:?} for which finality we were waiting is no longer included in the chain, aborting..."),
        };
    }
}
