use std::{
    collections::HashMap,
    str,
    str::{FromStr, Utf8Error},
};

use contracts_azero_client::{
    contract_transcode::{Value, Value::Seq},
    AccountId, Client, ClientWithSigner, ContractInstance, ExecCallParams, ReadonlyCallParams,
    Weight,
};
use log::{debug, error};
use thiserror::Error;

use crate::connections::azero::AzeroSigner;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum AzeroContractError {
    #[error("contract call error")]
    ContractCall(#[from] anyhow::Error),

    #[error("AzeroClient error")]
    AzeroClient(#[from] contracts_azero_client::ClientError),

    #[error("not account id")]
    NotAccountId(String),

    #[error("Invalid UTF-8 sequence")]
    InvalidUTF8(#[from] Utf8Error),

    #[error("Missing or invalid field")]
    MissingOrInvalidField(String),
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
        connection: &Client,
    ) -> Result<(bool, AccountId), AzeroContractError> {
        match self
            .contract
            .read0::<bool>(connection, "is_emergency", Default::default())
            .await
        {
            Ok(is_emergency) => Ok((is_emergency, self.address.clone())),
            Err(why) => Err(AzeroContractError::ContractCall(why)),
        }
    }
}

pub struct MostInstance {
    pub contract: ContractInstance,
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
            contract: ContractInstance::new(address, metadata_path)?,
            ref_time_limit,
            proof_size_limit,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn receive_request(
        &self,
        signed_connection: &ClientWithSigner<AzeroSigner>,
        request_hash: [u8; 32],
        committee_id: u128,
        dest_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> Result<(), AzeroContractError> {
        let gas_limit = Weight {
            ref_time: self.ref_time_limit,
            proof_size: self.proof_size_limit,
        };
        let args = [
            bytes32_to_str(&request_hash),
            committee_id.to_string(),
            bytes32_to_str(&dest_token_address),
            amount.to_string(),
            bytes32_to_str(&dest_receiver_address),
            request_nonce.to_string(),
        ];
        let params = ExecCallParams::new().gas_limit(gas_limit);

        // Exec does dry run first, so there's no need to repeat it here
        let call_result = self
            .contract
            .exec(signed_connection, "receive_request", &args, params)
            .await
            .map_err(AzeroContractError::ContractCall);
        debug!("receive_request: {:?}", call_result);
        call_result
    }

    pub async fn set_payout_account(
        &self,
        signed_connection: &ClientWithSigner<AzeroSigner>,
        committee_id: u128,
        payout_account: AccountId,
    ) -> Result<(), AzeroContractError> {
        let gas_limit = Weight {
            ref_time: self.ref_time_limit,
            proof_size: self.proof_size_limit,
        };
        let args = [committee_id.to_string(), payout_account.to_string()];
        let params = ExecCallParams::new().gas_limit(gas_limit);

        // Exec does dry run first, so there's no need to repeat it here
        let call_result = self
            .contract
            .exec(signed_connection, "set_payout_account", &args, params)
            .await
            .map_err(AzeroContractError::ContractCall);
        debug!("set_payout_account: {:?}", call_result);
        call_result
    }

    pub async fn is_halted(&self, connection: &Client) -> Result<bool, AzeroContractError> {
        Ok(self
            .contract
            .read0::<Result<bool, _>>(connection, "is_halted", Default::default())
            .await??)
    }

    pub async fn needs_signature(
        &self,
        connection: &Client,
        request_hash: [u8; 32],
        account: AccountId,
        committee_id: u128,
        block_finalized: bool,
    ) -> Result<bool, AzeroContractError> {
        let params = match block_finalized {
            false => ReadonlyCallParams::new(),
            true => {
                let finalized_hash = connection.get_finalized_block_hash().await?;
                ReadonlyCallParams::new().at(finalized_hash)
            }
        };
        Ok(self
            .contract
            .read(
                connection,
                "needs_signature",
                &[
                    bytes32_to_str(&request_hash),
                    account.to_string(),
                    committee_id.to_string(),
                ],
                params,
            )
            .await?)
    }

    pub async fn current_committee_id(
        &self,
        connection: &Client,
    ) -> Result<u128, AzeroContractError> {
        Ok(self
            .contract
            .read0::<Result<u128, _>>(connection, "get_current_committee_id", Default::default())
            .await??)
    }

    pub async fn is_in_committee(
        &self,
        connection: &Client,
        committee_id: u128,
        account: AccountId,
    ) -> Result<bool, AzeroContractError> {
        Ok(self
            .contract
            .read(
                connection,
                "is_in_committee",
                &[committee_id.to_string(), account.to_string()],
                Default::default(),
            )
            .await?)
    }
}

#[derive(Debug)]
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

fn bytes32_to_str(data: &[u8; 32]) -> String {
    "0x".to_owned() + &hex::encode(data)
}
