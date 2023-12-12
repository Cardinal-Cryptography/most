use std::{
    collections::HashMap,
    str,
    str::{FromStr, Utf8Error},
};

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance,
    },
    contract_transcode::{Value, Value::Seq},
    AccountId, AlephConfig, SignedConnection, TxInfo,
};
use log::trace;
use subxt::events::Events;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroContractError {
    #[error("aleph-client error")]
    AlephClient(#[from] anyhow::Error),

    #[error("not account id")]
    NotAccountId(String),

    #[error("Invalid UTF-8 sequence")]
    InvalidUTF8(#[from] Utf8Error),

    #[error("Missing or invalid field")]
    MissingOrInvalidField(String),
}

#[derive(Debug)]
pub struct MembraneInstance {
    pub contract: ContractInstance,
}

impl MembraneInstance {
    pub fn new(address: &str, metadata_path: &str) -> Result<Self, AzeroContractError> {
        let address = AccountId::from_str(address)
            .map_err(|why| AzeroContractError::NotAccountId(why.to_string()))?;
        Ok(Self {
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn receive_request(
        &self,
        signed_connection: &SignedConnection,
        request_hash: [u8; 32],
        dest_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        Ok(self
            .contract
            .contract_exec(
                signed_connection,
                "receive_request",
                &[
                    bytes32_to_str(&request_hash),
                    bytes32_to_str(&dest_token_address),
                    amount.to_string(),
                    bytes32_to_str(&dest_receiver_address),
                    request_nonce.to_string(),
                ],
            )
            .await?)
    }
}

pub struct CrosschainTransferRequestData {
    pub dest_token_address: [u8; 32],
    pub amount: u128,
    pub dest_receiver_address: [u8; 32],
    pub request_nonce: u128,
}

pub fn get_request_event_data(
    data: &HashMap<String, Value>,
) -> Result<CrosschainTransferRequestData, AzeroContractError> {
    let dest_token_address: [u8; 32] = decode_seq_field(data, "dest_token_address")?;
    let amount: u128 = decode_uint_field(data, "amount")?;
    let dest_receiver_address: [u8; 32] = decode_seq_field(data, "dest_receiver_address")?;
    let request_nonce: u128 = decode_uint_field(data, "request_nonce")?;

    Ok(CrosschainTransferRequestData {
        dest_token_address,
        amount,
        dest_receiver_address,
        request_nonce,
    })
}

pub fn filter_membrane_events(
    events: Events<AlephConfig>,
    membrane_instance: &MembraneInstance,
    block_details: BlockDetails,
) -> Vec<ContractEvent> {
    translate_events(
        events.iter(),
        &[&membrane_instance.contract],
        Some(block_details),
    )
    .into_iter()
    .filter_map(|event_res| {
        if let Ok(event) = event_res {
            Some(event)
        } else {
            trace!("Failed to translate event: {:?}", event_res);
            None
        }
    })
    .collect()
}

fn decode_seq_field(
    data: &HashMap<String, Value>,
    field: &str,
) -> Result<[u8; 32], AzeroContractError> {
    if let Some(Seq(seq_data)) = data.get(field) {
        match seq_data
            .elems()
            .iter()
            .try_fold(Vec::new(), |mut v, x| match x {
                Value::UInt(x) => {
                    v.push(*x as u8);
                    Ok(v)
                }
                _ => Err(AzeroContractError::MissingOrInvalidField(format!(
                    "Seq under data field {:?} contains elements of incorrect type",
                    field
                ))),
            })?
            .try_into()
        {
            Ok(x) => Ok(x),
            Err(_) => Err(AzeroContractError::MissingOrInvalidField(format!(
                "Seq under data field {:?} has incorrect length",
                field
            ))),
        }
    } else {
        Err(AzeroContractError::MissingOrInvalidField(format!(
            "Data field {:?} couldn't be found or has incorrect format",
            field
        )))
    }
}

fn decode_uint_field(
    data: &HashMap<String, Value>,
    field: &str,
) -> Result<u128, AzeroContractError> {
    if let Some(Value::UInt(x)) = data.get(field) {
        Ok(*x)
    } else {
        Err(AzeroContractError::MissingOrInvalidField(format!(
            "Data field {:?} couldn't be found or has incorrect format",
            field
        )))
    }
}

fn bytes32_to_str(data: &[u8; 32]) -> String {
    "0x".to_owned() + &hex::encode(data)
}
