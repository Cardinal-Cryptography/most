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
use ethers::{
    abi::Address,
    core::k256::{elliptic_curve::bigint::Zero, U256},
    types::BlockNumber,
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
    connections::{
        azero::{self, AzeroConnectionWithSigner, AzeroWsConnection},
        eth::SignedEthConnection,
    },
    contracts::{AzeroContractError, AzeroEtherInstance, MostInstance, RouterInstance, WETH9},
    helpers::left_pad,
    CircuitBreakerEvent,
};

// trader component will sell the surplus
pub const ONE_AZERO: u128 = 1_000_000_000_000;
pub const ONE_ETHER: u128 = 1_000_000_000_000_000_000;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum TraderError {
    #[error("Error when parsing ethereum address")]
    FromHex(#[from] rustc_hex::FromHexError),

    #[error("AlephClient error {0}")]
    AlephClient(#[from] anyhow::Error),

    #[error("Azero contract error {0}")]
    AzeroContract(#[from] AzeroContractError),

    #[error("Broadcast receive error {0}")]
    BroadcastReceive(#[from] broadcast::error::RecvError),

    #[error("Missing required arg {0}")]
    MissingRequired(String),

    #[error("Address is not an AccountId {0}")]
    NotAccountId(String),

    #[error("Flabbergasted {0}")]
    Unexpected(String),
}

#[derive(Copy, Clone)]
pub struct Trader;

impl Trader {
    pub async fn run(
        config: Arc<Config>,
        azero_signed_connection: &AzeroConnectionWithSigner,
        eth_signed_connection: Arc<SignedEthConnection>,
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
            azero_ether_metadata,
            eth_wrapped_ether_address,
            ..
        } = &*config;

        let most_azero = MostInstance::new(
            azero_contract_address,
            azero_contract_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        let router = RouterInstance::new(
            &router_address.clone().ok_or(TraderError::MissingRequired(
                "azero_wrapped_azero_address".to_owned(),
            ))?,
            router_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

        let azero_ether_address =
            azero_ether_address
                .clone()
                .ok_or(TraderError::MissingRequired(
                    "azero_ether_address".to_owned(),
                ))?;

        let azero_ether = AzeroEtherInstance::new(&azero_ether_address, azero_ether_metadata)?;

        let wrapped_azero_address =
            AccountId::from_str(&azero_wrapped_azero_address.clone().ok_or(
                TraderError::MissingRequired("azero_wrapped_azero_address".to_owned()),
            )?)
            .map_err(|err| TraderError::NotAccountId(err.to_owned()))?;

        let wrapped_ether_address = eth_wrapped_ether_address
            .clone()
            .ok_or(TraderError::MissingRequired(
                "eth_wrapped_ether_address".to_owned(),
            ))?
            .parse::<Address>()?;

        let wrapped_ether = WETH9::new(wrapped_ether_address, eth_signed_connection.clone());

        info!("Starting");

        loop {
            debug!("Ping");

            let whoami = azero_signed_connection.account_id();
            let azero_balance = azero_signed_connection
                .get_free_balance(whoami.to_owned(), None)
                .await;

            // check Azero balance
            if azero_balance > ONE_AZERO {
                let surplus = azero_balance.saturating_sub(ONE_AZERO);
                info!("{whoami} has {surplus} A0 above the set limit of {ONE_AZERO} A0 that will be swapped");

                let path = [wrapped_azero_address.clone(), azero_ether.address.clone()];

                let amounts_out = match router
                    .get_amounts_out(azero_signed_connection.as_connection(), surplus, &path)
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
                        azero_signed_connection,
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
            let azero_eth_balance = azero_ether
                .balance_of(azero_signed_connection.as_connection(), whoami.clone())
                .await?;

            let mut receiver: [u8; 32] = [0; 32];

            receiver.copy_from_slice(&left_pad(eth_signed_connection.address().0.to_vec(), 32));

            if azero_eth_balance > ONE_ETHER {
                if let Err(why) = most_azero
                    .send_request(
                        azero_signed_connection,
                        *azero_ether.address.clone().as_ref(),
                        azero_eth_balance,
                        receiver,
                    )
                    .await
                {
                    warn!("Could not request cross chain transfer: {why:?}");
                    continue;
                }
            }

            // check 0xwETH balance
            let balance = match wrapped_ether
                .balance_of(eth_signed_connection.address())
                .block(BlockNumber::Finalized)
                .await
            {
                Ok(balance) => balance,
                Err(why) => {
                    warn!("Query for WETH balance failed : {why:?}");
                    continue;
                }
            };

            // withdraw 0xwETH -> ETH
            if !balance.is_zero() {
                if let Err(why) = wrapped_ether
                    .withdraw(balance)
                    .block(BlockNumber::Finalized)
                    .await
                {
                    warn!("Unwrapping WETH failed : {why:?}");
                    continue;
                }
            }
        }
    }
}
