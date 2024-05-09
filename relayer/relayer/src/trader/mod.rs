use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use aleph_client::{pallets::system::SystemApi, AccountId, AsConnection, SignedConnectionApi};
use ethers::{abi::Address, providers::Middleware, types::BlockNumber};
use log::{debug, error, info, warn};
use thiserror::Error;
use tokio::{select, sync::broadcast, time::sleep};

use crate::{
    config::Config,
    connections::{azero::AzeroConnectionWithSigner, eth::SignedEthConnection},
    contracts::{AzeroContractError, AzeroEtherInstance, MostInstance, RouterInstance, WETH9},
    helpers::left_pad,
    CircuitBreakerEvent,
};

// trader component will sell the surplus
pub const ONE_AZERO: u128 = 1_000_000_000_000;
pub const ONE_ETHER: u128 = 1_000_000_000_000_000_000;

pub const ETH_TO_AZERO_RELAYING_BUFFER: u128 = 100 * ONE_AZERO;
pub const TRADED_AZERO_FEE_MULTIPLIER: u128 = 20;
pub const SLIPPAGE_PERCENT: u128 = 1;

pub const HOUR_IN_MILLIS: u64 = 60 * 60 * 1000;
pub const TRADER_QUERY_INTERVAL_MINS: u64 = 5;

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum TraderError {
    #[error("Error when parsing ethereum address {0}")]
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

    #[error("Trader has exited unexpectedly - this should never happen")]
    TraderExited,
}

#[derive(Copy, Clone)]
pub struct Trader;

impl Trader {
    pub async fn run(
        config: Arc<Config>,
        azero_signed_connection: Arc<AzeroConnectionWithSigner>,
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
            &router_address
                .clone()
                .ok_or(TraderError::MissingRequired("router_address".to_owned()))?,
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

        let azero_ether = AzeroEtherInstance::new(
            &azero_ether_address,
            azero_ether_metadata,
            *azero_ref_time_limit,
            *azero_proof_size_limit,
        )?;

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

        let whoami_azero = azero_signed_connection.account_id();
        let whoami_eth = eth_signed_connection.address().to_string();

        let swap_path = [wrapped_azero_address.clone(), azero_ether.address.clone()];

        let mut receiver: [u8; 32] = [0; 32];
        receiver.copy_from_slice(&left_pad(eth_signed_connection.address().0.to_vec(), 32));

        info!("Starting");

        select! {
            cb_event = circuit_breaker_receiver.recv() => {
                warn!("Exiting due to a circuit breaker event {cb_event:?}.");
                Ok(cb_event?)
            },

            () = async {
                loop {
                    debug!("Ping");

                    let azero_balance = azero_signed_connection
                        .get_free_balance(whoami_azero.to_owned(), None)
                        .await;

                    info!("{whoami_azero} has a balance of: {azero_balance} Azero.");

                    let current_base_fee = match most_azero.get_base_fee(azero_signed_connection.as_connection()).await {
                        Ok(amount) => {
                            info!("Current base fee: {amount} pA0");
                            amount},
                        Err(why) => {
                            warn!("Query to `get_base_fee` has failed {why:?}.");
                            continue;
                        },
                    };

                    let azero_available_for_swap = azero_balance.saturating_sub(current_base_fee + ETH_TO_AZERO_RELAYING_BUFFER);

                    // check azero balance
                    if azero_available_for_swap > current_base_fee * TRADED_AZERO_FEE_MULTIPLIER {
                        info!("{azero_available_for_swap} A0 above the safe limit will be swapped.");

                        let min_weth_amount_out = match router
                            .get_amounts_out(azero_signed_connection.as_connection(), azero_available_for_swap, &swap_path)
                            .await
                        {
                            Ok(amounts) => {
                                debug!("Amounts out {amounts:?}.");

                                match amounts.last() {
                                    Some(amount) => amount.saturating_mul(100 - SLIPPAGE_PERCENT).saturating_div(100),
                                    None => {
                                        warn!("Query to `calculate_amounts_out` returned an empty result.");
                                        continue;
                                    }
                                }
                            },

                            Err(why) => {
                                warn!("Could not `get_amounts_out`: {why:?}");
                                continue;
                            }
                        };

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("unix timestamp")
                            .as_millis();

                        info!("Requesting a swap of {azero_available_for_swap} Azero to at least {min_weth_amount_out} Azero ETH.");

                        if let Err(why) = router
                            .swap_exact_native_for_tokens(
                                &azero_signed_connection,
                                azero_available_for_swap,
                                min_weth_amount_out,
                                &swap_path,
                                whoami_azero.clone(),
                                now.saturating_add(HOUR_IN_MILLIS.into()) as u64, // within one hour
                            )
                            .await
                        {
                            warn!("Could not perform swap: {why:?}.");
                            continue;
                        }

                        // check azero Eth balance
                        let azero_eth_balance = match azero_ether
                            .balance_of(azero_signed_connection.as_connection(), whoami_azero.clone())
                            .await {
                                Ok(balance) => balance,
                                Err(why) => {
                                    warn!("Error when querying for Azero ETH balance: {why:?}.");
                                    continue;
                                },
                            };

                        if azero_eth_balance > 0 {
                            info!("Requesting a cross chain transfer of {azero_eth_balance} units of Azero ETH [{azero_ether_address}] to {whoami_eth}.");

                            // set allowance
                            if let Err(why) = azero_ether.approve(&azero_signed_connection, most_azero.address.clone(), azero_eth_balance).await {
                                warn!("Approve tx failed: {why:?}.");
                                continue;
                            }

                            if let Err(why) = most_azero
                                .send_request(
                                    &azero_signed_connection,
                                    *azero_ether.address.clone().as_ref(),
                                    azero_eth_balance,
                                    receiver,
                                    current_base_fee
                                )
                                .await
                            {
                                warn!("Could not send the cross-chain transfer request: {why:?}.");
                                continue;
                            }

                        }

                    } else {
                        debug!("{whoami_azero} has A0 balance too low for bridging");
                    }

                    // check 0xwETH balance
                    let wrapped_ether_balance = match wrapped_ether
                        .balance_of(eth_signed_connection.address())
                        .block(BlockNumber::Finalized)
                        .await
                    {
                        Ok(balance) => balance,
                        Err(why) => {
                            warn!("Query for WETH balance failed : {why:?}.");
                            continue;
                        }
                    };

                    debug!("{whoami_eth} has a balance of: {wrapped_ether_balance:?} WETH9.");

                    // withdraw 0xwETH -> ETH
                    if !wrapped_ether_balance.is_zero() {

                        info!("{whoami_eth} has a positive balance of {wrapped_ether_balance} WETH9 that will be unwrapped.");

                        if let Err(why) = wrapped_ether
                            .withdraw(wrapped_ether_balance)
                            .block(BlockNumber::Finalized)
                            .await {
                                warn!("Unwrapping WETH failed : {why:?}.");
                                continue;
                            }
                    }

                    // check ETH balance
                    if let Ok (eth_balance) = eth_signed_connection.get_balance(eth_signed_connection.address(), None).await {
                        // warning if the balance drops too low
                        if eth_balance < ONE_ETHER.into () {
                            warn!("{whoami_eth} has a low ETH balance: {eth_balance} ETH.");
                        } else {
                            info!("{whoami_eth} has a balance of {eth_balance} ETH.");
                        }
                    }

                    sleep(Duration::from_mins(TRADER_QUERY_INTERVAL_MINS)).await;
                }

            } => {
                Err(TraderError::TraderExited)
            }
        }
    }
}
