use std::{cmp::min, str::FromStr, sync::Arc, time::Duration};

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
    #[error("Azero contract error {0}")]
    AzeroContract(#[from] AzeroContractError),

    // #[error("broadcast send error")]
    // BroadcastSend(#[from] broadcast::error::SendError<CircuitBreakerEvent>),
    #[error("broadcast receive error {0}")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("missing required arg {0}")]
    MissingRequired(String),

    #[error("flabbergasted {0}")]
    Unexpected(String),
}

#[derive(Copy, Clone)]
pub struct Trader;

impl Trader {
    pub async fn run(
        config: Arc<Config>,
        azero_connection: &AzeroConnectionWithSigner,
        circuit_breaker_sender: broadcast::Sender<CircuitBreakerEvent>,
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

                let path0 =
                    azero_wrapped_azero_address
                        .clone()
                        .ok_or(TraderError::MissingRequired(
                            "azero_wrapped_azero_address".to_owned(),
                        ))?;
                let path1 = azero_ether_address
                    .clone()
                    .ok_or(TraderError::MissingRequired(
                        "azero_ether_address".to_owned(),
                    ))?;

                let amounts_out = match router
                    .calculate_amounts_out(
                        azero_connection.as_connection(),
                        surplus,
                        &[
                            AccountId::from_str(&path0)
                                .map_err(|err| TraderError::Unexpected(err.to_owned()))?,
                            AccountId::from_str(&path1)
                                .map_err(|err| TraderError::Unexpected(err.to_owned()))?,
                        ],
                    )
                    .await
                {
                    Ok(amounts) => amounts,
                    Err(why) => {
                        warn!("Cannot calculate amounts_out: {why:?}");
                        continue;
                    }
                };

                let weth_amount_out = match amounts_out.last() {
                    Some(_) => todo!(),
                    None => {
                        warn!("Query returned an empty result");
                        continue;
                    }
                };

                // 0.5 percent slippage
                let min_weth_amount_out = weth_amount_out.saturating_mul(995).saturating_div(1000);

                // fn swap_exact_native_for_tokens(
                //     &mut self,
                //     amount_out_min: u128,
                //     path: Vec<AccountId>,
                //     to: AccountId,
                //     deadline: u64,
                // )

                // if let Err(why) = wrapped_azero.deposit(azero_connection, surplus).await {
                //     warn!("Failed to wrap {surplus} A0 as wrappedAzero: {why:?}");
                // }
            }

            // check wAzero balance
            // let wazero_balance = wrapped_azero
            //     .balance_of(azero_connection.as_connection(), whoami.to_owned())
            //     .await?;

            // TODO approve
            // TODO swap all wAzero to wETH

            // swap_exact_native_for_tokens

            // TODO bridge wETH to ETHEREUM

            // TODO unwrap 0xWETH -> ETH

            // select! {
            //     cb_event = circuit_breaker_receiver.recv () => {
            //         warn!("Exiting due to a circuit breaker event {cb_event:?}");
            //         return Ok(cb_event?);
            //     },

            // }
        }
    }
}
