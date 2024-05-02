use std::{
    cmp::min,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aleph_client::{
    contract::event::{BlockDetails, ContractEvent},
    pallets::system::SystemApi,
    utility::BlocksApi,
    AccountId, AlephConfig, AsConnection, Connection, SignedConnectionApi,
};
use futures::stream::{FuturesOrdered, StreamExt};
use log::{debug, error, info, warn};
use subxt::{events::Events, storage::address};
use thiserror::Error;
use tokio::{
    select,
    sync::{broadcast, mpsc, oneshot},
    task::{JoinError, JoinSet},
    time::sleep,
};

use super::AzeroMostEvents;
use crate::{
    config::Config,
    connections::azero::{AzeroConnectionWithSigner, AzeroWsConnection},
    contracts::{AzeroContractError, MostInstance, RouterInstance},
    CircuitBreakerEvent,
};

// trader component will sell the surplus
pub const AZERO_SURPLUS_LIMIT: u128 = 1_000_000_000_000; // 1 AZERO

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum TraderError {
    #[error("AlephClient error {0}")]
    AlephClient(#[from] anyhow::Error),

    #[error("Azero contract error {0}")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Broadcast receive error {0}")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("Missing required arg {0}")]
    MissingRequired(String),

    #[error("Flabbergasted {0}")]
    Unexpected(String),
}

#[derive(Copy, Clone)]
pub struct Trader;

impl Trader {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: &AzeroConnectionWithSigner,
        // circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
        mut circuit_breaker_receiver: broadcast::Receiver<CircuitBreakerEvent>,
    ) -> Result<CircuitBreakerEvent, TraderError> {
        let Config {
            azero_contract_metadata,
            azero_contract_address,
            azero_ref_time_limit,
            azero_proof_size_limit,
            router_address,
            router_metadata,
            azero_wrapped_azero_address,
            azero_ether_address,
            eth_wrapped_ether_address,
            ..
        } = &*config;

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        let router_address = &router_address.clone().ok_or(TraderError::MissingRequired(
            "azero_wrapped_azero_address".to_owned(),
        ))?;

        let router = RouterInstance::new(
            router_address,
            router_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        info!("Starting");

        loop {
            debug!("Ping");

            let whoami = azero_connection.account_id();
            let azero_balance = azero_connection
                .get_free_balance(whoami.to_owned(), None)
                .await;

            // check Azero balance
            if azero_balance > AZERO_SURPLUS_LIMIT {
                let surplus = azero_balance.saturating_sub(AZERO_SURPLUS_LIMIT);
                info!("{whoami} has {surplus} A0 above the set limit of {AZERO_SURPLUS_LIMIT} A0 that will be swapped");

                let wrapped_azero_address =
                    AccountId::from_str(&azero_wrapped_azero_address.clone().ok_or(
                        TraderError::MissingRequired("azero_wrapped_azero_address".to_owned()),
                    )?)
                    .map_err(|err| TraderError::Unexpected(err.to_owned()))?;

                let azero_ether_address = AccountId::from_str(&azero_ether_address.clone().ok_or(
                    TraderError::MissingRequired("azero_ether_address".to_owned()),
                )?)
                .map_err(|err| TraderError::Unexpected(err.to_owned()))?;

                let path = [wrapped_azero_address.clone(), azero_ether_address.clone()];

                let amounts_out = match router
                    .get_amounts_out(azero_connection.as_connection(), surplus, &path)
                    .await
                {
                    Ok(amounts) => amounts,
                    Err(why) => {
                        warn!("Cannot calculate amounts_out: {why:?}");
                        continue;
                    }
                };

                let min_weth_amount_out = match amounts_out.last() {
                    Some(amount) => amount.saturating_mul(995).saturating_div(1000), // 0.5 percent slippage
                    None => {
                        warn!("Query to `calculate_amounts_out` returned an empty result");
                        continue;
                    }
                };

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("unix timestamp")
                    .as_millis();

                if let Err(why) = router
                    .swap_exact_native_for_tokens(
                        azero_connection,
                        surplus,
                        min_weth_amount_out,
                        &path,
                        whoami.clone(),
                        now.saturating_add(3600000) as u64, // one hour
                    )
                    .await
                {
                    warn!("Could not perform the swap: {why:?}");
                    continue;
                }
            }

            // check azero Eth balance

            // TODO bridge wETH to ETHEREUM

            // TODO unwrap 0xWETH -> ETH
        }
    }
}
