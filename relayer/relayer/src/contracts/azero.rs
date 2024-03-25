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
    contract_transcode::{ContractMessageTranscoder, Value, Value::Seq},
    pallets::contract::{ContractCallArgs, ContractRpc, ContractsUserApi},
    sp_weights::weight_v2::Weight,
    AccountId, AlephConfig, AsConnection, Connection, SignedConnectionApi, TxInfo, TxStatus,
};
use log::trace;
use subxt::events::Events;
use thiserror::Error;

use crate::{connections::azero::AzeroConnectionWithSigner, contracts::azero::Value::Tuple};

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

    #[error("Dry run failed")]
    DryRunFailed(String),
}

pub struct AdvisoryInstance {
    pub contract: ContractInstance,
    pub address: AccountId,
}

impl AdvisoryInstance {
    pub fn new(address: &str, metadata_path: &str) -> Result<Self, AzeroContractError> {
        let address = AccountId::from_str(address)
            .map_err(|why| AzeroContractError::NotAccountId(why.to_string()))?;
        Ok(Self {
            address: address.clone(),
            contract: ContractInstance::new(address, metadata_path)?,
        })
    }

    pub async fn is_emergency(
        &self,
        connection: &Connection,
    ) -> Result<(bool, AccountId), AzeroContractError> {
        match self
            .contract
            .contract_read0::<bool, _>(connection, "is_emergency")
            .await
        {
            Ok(is_emergency) => Ok((is_emergency, self.address.clone())),
            Err(why) => Err(AzeroContractError::AlephClient(why)),
        }
    }
}

pub struct MostInstance {
    pub contract: ContractInstance,
    pub address: AccountId,
    pub transcoder: ContractMessageTranscoder,
    pub ref_time_limit: u64,
    pub proof_size_limit: u64,
}

impl MostInstance {
    pub fn new(
        address: &str,
        metadata_path: &str,
        ref_time_limit: u64,
        proof_size_limit: u64,
    ) -> Result<Self, AzeroContractError> {
        let address = AccountId::from_str(address)
            .map_err(|why| AzeroContractError::NotAccountId(why.to_string()))?;
        Ok(Self {
            address: address.clone(),
            transcoder: ContractMessageTranscoder::load(metadata_path)?,
            contract: ContractInstance::new(address, metadata_path)?,
            ref_time_limit,
            proof_size_limit,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn receive_request(
        &self,
        signed_connection: &AzeroConnectionWithSigner,
        request_hash: [u8; 32],
        committee_id: u128,
        dest_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        let args = [
            bytes32_to_str(&request_hash),
            committee_id.to_string(),
            bytes32_to_str(&dest_token_address),
            amount.to_string(),
            bytes32_to_str(&dest_receiver_address),
            request_nonce.to_string(),
        ];

        let data = self.transcoder.encode("receive_request", args)?;
        // [Audit] How about checking gas price also for Aleph Zero?
        signed_connection
            .call(
                self.address.clone(),
                0,
                Weight {
                    ref_time: self.ref_time_limit,
                    proof_size: self.proof_size_limit,
                },
                None,
                data,
                TxStatus::Finalized,
            )
            .await
            .map_err(AzeroContractError::AlephClient)
    }

    pub async fn is_halted(
        &self,
        connection: &AzeroConnectionWithSigner,
    ) -> Result<bool, AzeroContractError> {
        let data = self.transcoder.encode::<_, &str>("is_halted", [])?;
        let args = ContractCallArgs {
            origin: connection.account_id().clone(),
            dest: self.address.clone(),
            value: 0,
            gas_limit: None,
            input_data: data,
            storage_deposit_limit: None,
        };

        let contract_read_result = connection.as_connection().call_and_get(args).await?;

        if let Ok(res) = &contract_read_result.result {
            match self
                .transcoder
                .decode_return("is_halted", &mut res.data.as_slice())?
            {
                Tuple(tuple) => {
                    if tuple.ident() == Some("Ok".to_string()) {
                        Ok(decode_bool_from_tuple(
                            tuple.values().collect::<Vec<_>>()[0],
                        )?)
                    } else if tuple.ident() == Some("Err".to_string()) {
                        Err(AzeroContractError::DryRunFailed(format!(
                            "{:?}",
                            tuple.values().collect::<Vec<_>>()
                        )))
                    } else {
                        Err(AzeroContractError::DryRunFailed(
                            "Invalid response".to_string(),
                        ))
                    }
                }
                _ => Err(AzeroContractError::DryRunFailed(
                    "Invalid response".to_string(),
                )),
            }
        } else {
            Err(AzeroContractError::DryRunFailed(format!(
                "{:?}",
                contract_read_result
            )))
        }
    }

    pub fn filter_events(
        &self,
        events: Events<AlephConfig>,
        block_details: BlockDetails,
    ) -> Vec<ContractEvent> {
        translate_events(events.iter(), &[&self.contract], Some(block_details))
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
}

pub struct CrosschainTransferRequestData {
    pub committee_id: u128,
    pub dest_token_address: [u8; 32],
    pub amount: u128,
    pub dest_receiver_address: [u8; 32],
    pub request_nonce: u128,
}

pub fn get_request_event_data(
    data: &HashMap<String, Value>,
) -> Result<CrosschainTransferRequestData, AzeroContractError> {
    let committee_id: u128 = decode_uint_field(data, "committee_id")?;
    let dest_token_address: [u8; 32] = decode_seq_field(data, "dest_token_address")?;
    let amount: u128 = decode_uint_field(data, "amount")?;
    let dest_receiver_address: [u8; 32] = decode_seq_field(data, "dest_receiver_address")?;
    let request_nonce: u128 = decode_uint_field(data, "request_nonce")?;

    Ok(CrosschainTransferRequestData {
        committee_id,
        dest_token_address,
        amount,
        dest_receiver_address,
        request_nonce,
    })
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

fn decode_bool_from_tuple(data: &Value) -> Result<bool, AzeroContractError> {
    if let Tuple(tuple_val) = data {
        if let Value::Bool(x) = tuple_val.values().collect::<Vec<_>>()[0] {
            Ok(*x)
        } else {
            Err(AzeroContractError::DryRunFailed(format!(
                "Value {:?} couldn't be decoded as bool",
                data
            )))
        }
    } else {
        Err(AzeroContractError::DryRunFailed(format!(
            "Value {:?} couldn't be decoded as bool",
            data
        )))
    }
}

fn bytes32_to_str(data: &[u8; 32]) -> String {
    "0x".to_owned() + &hex::encode(data)
}
