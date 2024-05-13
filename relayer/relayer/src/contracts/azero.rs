use std::{
    collections::HashMap,
    str,
    str::{FromStr, Utf8Error},
};

use aleph_client::{
    contract::{
        event::{translate_events, BlockDetails, ContractEvent},
        ContractInstance, ExecCallParams, ReadonlyCallParams,
    },
    contract_transcode::Value::{self, Seq},
    sp_weights::weight_v2::Weight,
    utility::BlocksApi,
    waiting::BlockStatus,
    AccountId, AlephConfig, Connection, SignedConnectionApi, TxInfo,
};
use log::{debug, error, trace};
use subxt::events::Events;
use thiserror::Error;

use crate::connections::azero::AzeroConnectionWithSigner;

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

    #[error("send_request tx has failed")]
    SendRequestTxFailure {
        src_token_address: String,
        amount: u128,
        dest_receiver_address: String,
        base_fee: u128,
    },
}

pub struct RouterInstance {
    pub contract: ContractInstance,
    pub address: AccountId,
    pub ref_time_limit: u64,
    pub proof_size_limit: u64,
}

impl RouterInstance {
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
            contract: ContractInstance::new(address, metadata_path)?,
            ref_time_limit,
            proof_size_limit,
        })
    }

    pub async fn swap_exact_native_for_tokens(
        &self,
        signed_connection: &AzeroConnectionWithSigner,
        amount_in: u128,
        amount_out_min: u128,
        path: &[AccountId],
        to: AccountId,
        deadline: u64,
    ) -> Result<TxInfo, AzeroContractError> {
        let gas_limit = Weight {
            ref_time: self.ref_time_limit,
            proof_size: self.proof_size_limit,
        };

        let path_encoding = self.encode_vec(path);
        let args: Vec<String> = vec![
            amount_out_min.to_string(),
            path_encoding,
            to.to_string(),
            deadline.to_string(),
        ];

        let params = ExecCallParams::new().gas_limit(gas_limit).value(amount_in);
        self.contract
            .exec(
                signed_connection,
                "Router::swap_exact_native_for_tokens",
                &args,
                params,
            )
            .await
            .map_err(AzeroContractError::AlephClient)
    }

    pub async fn get_amounts_out(
        &self,
        connection: &Connection,
        amount_in: u128,
        path: &[AccountId],
    ) -> Result<Vec<u128>, AzeroContractError> {
        let path_encoding = self.encode_vec(path);
        Ok(self
            .contract
            .read::<_, Result<Vec<u128>, _>, _>(
                connection,
                "Router::get_amounts_out",
                &[amount_in.to_string(), path_encoding],
                Default::default(),
            )
            .await??)
    }

    fn encode_vec<T>(&self, coll: &[T]) -> String
    where
        T: ToString,
    {
        let mut encoding: String = "[".to_owned();
        for i in 0..coll.len() {
            encoding.push_str(&coll[i].to_string());
            if i < (coll.len() - 1) {
                encoding.push(',')
            }
        }
        encoding.push(']');
        encoding
    }
}

pub struct AzeroEtherInstance {
    pub contract: ContractInstance,
    pub address: AccountId,
    pub ref_time_limit: u64,
    pub proof_size_limit: u64,
}

impl AzeroEtherInstance {
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
            contract: ContractInstance::new(address, metadata_path)?,
            ref_time_limit,
            proof_size_limit,
        })
    }

    pub async fn approve(
        &self,
        signed_connection: &AzeroConnectionWithSigner,
        spender: AccountId,
        value: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        let gas_limit = Weight {
            ref_time: self.ref_time_limit,
            proof_size: self.proof_size_limit,
        };
        let args = [spender.to_string(), value.to_string()];
        let params = ExecCallParams::new().gas_limit(gas_limit);

        // Exec does dry run first, so there's no need to repeat it here
        self.contract
            .exec(signed_connection, "PSP22::approve", &args, params)
            .await
            .map_err(AzeroContractError::AlephClient)
    }

    pub async fn balance_of(
        &self,
        connection: &Connection,
        owner: AccountId,
    ) -> Result<u128, AzeroContractError> {
        Ok(self
            .contract
            .read(
                connection,
                "PSP22::balance_of",
                &[owner.to_string()],
                Default::default(),
            )
            .await?)
    }
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
            .read0::<bool, _>(connection, "is_emergency", Default::default())
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
            contract: ContractInstance::new(address, metadata_path)?,
            ref_time_limit,
            proof_size_limit,
        })
    }

    pub async fn get_base_fee(&self, connection: &Connection) -> Result<u128, AzeroContractError> {
        Ok(self
            .contract
            .read0::<Result<u128, _>, _>(connection, "get_base_fee", Default::default())
            .await??)
    }

    pub async fn get_collected_reward(
        &self,
        connection: &Connection,
        committee_id: u128,
        member_id: AccountId,
    ) -> Result<u128, AzeroContractError> {
        Ok(self
            .contract
            .read(
                connection,
                "get_outstanding_member_rewards",
                &[committee_id.to_string(), member_id.to_string()],
                Default::default(),
            )
            .await?)
    }

    pub async fn payout_rewards(
        &self,
        signed_connection: &AzeroConnectionWithSigner,
        committee_id: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        let args = [
            committee_id.to_string(),
            signed_connection.account_id().to_string(),
        ];

        self.contract
            .exec(
                signed_connection,
                "payout_rewards",
                &args,
                ExecCallParams::new(),
            )
            .await
            .map_err(AzeroContractError::AlephClient)
    }

    pub async fn send_request(
        &self,
        signed_connection: &AzeroConnectionWithSigner,
        src_token_address: [u8; 32],
        amount: u128,
        dest_receiver_address: [u8; 32],
        base_fee: u128,
    ) -> Result<TxInfo, AzeroContractError> {
        let gas_limit = Weight {
            ref_time: self.ref_time_limit,
            proof_size: self.proof_size_limit,
        };
        let args = [
            bytes32_to_str(&src_token_address),
            amount.to_string(),
            bytes32_to_str(&dest_receiver_address),
        ];

        let params = ExecCallParams::new().gas_limit(gas_limit).value(base_fee);

        // Exec does dry run first, so there's no need to repeat it here
        self.contract
            .exec(signed_connection, "send_request", &args, params)
            .await
            .map_err(|why| {
                error!("send_request failure: {why:?}");
                AzeroContractError::SendRequestTxFailure {
                    src_token_address: hex::encode(src_token_address),
                    amount,
                    dest_receiver_address: hex::encode(dest_receiver_address),
                    base_fee,
                }
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
            .map_err(AzeroContractError::AlephClient);
        debug!("receive_request: {:?}", call_result);
        call_result
    }

    pub async fn is_halted(&self, connection: &Connection) -> Result<bool, AzeroContractError> {
        Ok(self
            .contract
            .read0::<Result<bool, _>, _>(connection, "is_halted", Default::default())
            .await??)
    }

    pub async fn needs_signature(
        &self,
        connection: &Connection,
        request_hash: [u8; 32],
        account: AccountId,
        committee_id: u128,
        block: BlockStatus,
    ) -> Result<bool, AzeroContractError> {
        let params = match block {
            BlockStatus::Best => ReadonlyCallParams::new(),
            BlockStatus::Finalized => {
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
        connection: &Connection,
    ) -> Result<u128, AzeroContractError> {
        Ok(self
            .contract
            .read0::<Result<u128, _>, _>(connection, "get_current_committee_id", Default::default())
            .await??)
    }

    pub async fn is_in_committee(
        &self,
        connection: &Connection,
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
