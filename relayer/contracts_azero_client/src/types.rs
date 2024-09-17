use std::collections::HashMap;

use contract_transcode::Value;
pub use subxt::utils::{H160, H256};
use subxt::{
    blocks::ExtrinsicEvents,
    events::StaticEvent,
    ext::{
        codec::{Decode, Encode, Error, Input},
        scale_decode::DecodeAsType,
        sp_core::{ed25519, sr25519},
    },
    utils::AccountId32,
    PolkadotConfig,
};

pub type MultiSignature = subxt::utils::MultiSignature;
/// An alias for a pallet aleph keys.
pub type AlephKeyPair = ed25519::Pair;
/// An alias for a type of a key pair that signs chain transactions.
pub type RawKeyPair = sr25519::Pair;

pub type AccountId = AccountId32;
pub type BlockHash = H256;
pub type Balance = u128;

#[derive(Decode, Encode, Clone, Debug, Eq, PartialEq)]
pub struct Weight {
    #[codec(compact)]
    pub ref_time: u64,
    #[codec(compact)]
    pub proof_size: u64,
}

impl Weight {
    pub fn new(ref_time: u64, proof_size: u64) -> Self {
        Self {
            ref_time,
            proof_size,
        }
    }
}

/// Represents a single event emitted by a contract.
///
/// It does not have an
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ContractEvent {
    /// The address of the contract that emitted the event.
    pub contract: AccountId,
    /// The name of the event.
    pub name: Option<String>,
    /// Data contained in the event.
    pub data: HashMap<String, Value>,
}

#[derive(Encode)]
pub struct ContractCallArgs {
    /// Who is singing a tx.
    pub origin: AccountId,
    /// Address of the contract to call.
    pub dest: AccountId,
    /// The balance to transfer from the `origin` to `dest`.
    pub value: Balance,
    /// The gas limit enforced when executing the constructor.
    pub gas_limit: Option<Weight>,
    /// The maximum amount of balance that can be charged from the caller to pay for the storage consumed.
    pub storage_deposit_limit: Option<Balance>,
    /// The input data to pass to the contract.
    pub input_data: Vec<u8>,
}

/// This is a stub for a Struct we do not use, and cant construct without knowning a prio how runtime
/// looks like.
pub struct EventRecord;

impl Decode for EventRecord {
    fn decode<I: Input>(input: &mut I) -> Result<Self, Error> {
        let len = input.remaining_len()?;
        // drain remaining input
        match len {
            Some(len) => {
                let mut buf = vec![0; len];
                input.read(&mut buf)?;
            }
            _ => while input.read_byte().is_ok() {},
        }

        Ok(EventRecord)
    }
}

/// Event definition from the pallet_contracts
#[derive(Decode, Encode, DecodeAsType)]
pub struct ContractEmitted {
    pub contract: AccountId,
    pub data: Vec<u8>,
}

impl StaticEvent for ContractEmitted {
    const PALLET: &'static str = "Contracts";
    const EVENT: &'static str = "ContractEmitted";
}

/// Data regarding submitted transaction.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TxInfo {
    /// Hash of the transaction itself.
    pub tx_hash: BlockHash,
}

impl From<ExtrinsicEvents<PolkadotConfig>> for TxInfo {
    fn from(ee: ExtrinsicEvents<PolkadotConfig>) -> Self {
        Self {
            tx_hash: ee.extrinsic_hash(),
        }
    }
}
