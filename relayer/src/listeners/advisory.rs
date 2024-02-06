use std::{cmp::min, collections::BTreeSet, sync::Arc};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    utility::BlocksApi,
    AlephConfig, AsConnection,
};
use crossbeam_channel::Sender;
use ethers::{
    abi::{self, EncodePackedError, Token},
    core::types::Address,
    prelude::{ContractCall, ContractError},
    providers::{Middleware, ProviderError},
    utils::keccak256,
};
use log::{debug, error, info, warn};
use redis::{aio::Connection as RedisConnection, RedisError};
use subxt::{events::Events, utils::H256};
use thiserror::Error;
use tokio::{
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
    time::{sleep, Duration},
};

use super::AzeroListenerError;
use crate::{
    config::Config,
    connections::{
        azero::{AzeroWsConnection, SignedAzeroWsConnection},
        eth::{EthConnection, EthConnectionError, SignedEthConnection},
        redis_helpers::{read_first_unprocessed_block_number, write_last_processed_block},
    },
    contracts::{
        get_request_event_data, AdvisoryInstance, AzeroContractError,
        CrosschainTransferRequestData, Most, MostInstance,
    },
    listeners::eth::{get_next_finalized_block_number_eth, ETH_BLOCK_PROD_TIME_SEC},
};

pub struct AdvisoryListener;

impl AdvisoryListener {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: Arc<AzeroWsConnection>,
        sender: Sender<bool>,
    ) -> Result<(), AzeroListenerError> {
        let Config {
            advisory_contract_metadata,
            advisory_contract_address,
            ..
        } = &*config;

        let advisory_instance = AdvisoryInstance::new(
            &advisory_contract_address.clone().expect("Advisory address"),
            advisory_contract_metadata,
        )?;

        let is_emergency = advisory_instance.is_emergency(&azero_connection);

        // TODO: send to channel

        // TODO : listener

        todo!()
    }
}
