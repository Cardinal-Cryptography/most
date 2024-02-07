use std::{env, process, sync::Arc};

use aleph_client::pallets::balances::BalanceUserApi;
use clap::Parser;
use config::Config;
use connections::EthConnectionError;
use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder, Signer, WalletError};
use eyre::Result;
use log::{debug, error, info};
use redis::Client as RedisClient;
use thiserror::Error;
use tokio::{runtime::Runtime, sync::Mutex};

use crate::{
    connections::{
        azero::{self, AzeroConnectionWithSigner},
        eth,
    },
    listeners::{AlephZeroListener, AzeroListenerError, EthListener, EthListenerError},
};

mod config;
mod connections;
mod contracts;
mod helpers;
mod listeners;

const DEV_MNEMONIC: &str =
    "harsh master island dirt equip search awesome double turn crush wool grant";

#[derive(Debug, Error)]
#[error(transparent)]
#[non_exhaustive]
pub enum ListenerError {
    #[error("eth listener error")]
    Eth(#[from] EthListenerError),

    #[error("eth provider connection error")]
    EthConnection(#[from] EthConnectionError),

    #[error("eth wallet error")]
    EthWallet(#[from] WalletError),

    #[error("eth listener error")]
    Azero(#[from] AzeroListenerError),
}

fn main() -> Result<()> {
    let config = Arc::new(Config::parse());

    env::set_var("RUST_LOG", config.rust_log.as_str());
    env_logger::init();

    info!("{:#?}", &config);

    let rt = Runtime::new()?;

    rt.block_on(async {
        let mut tasks = Vec::with_capacity(4);

        let client = RedisClient::open(config.redis_node.clone())
            .expect("Cannot connect to the redis cluster instance");
        let redis_connection = Arc::new(Mutex::new(client.get_async_connection().await.unwrap()));

        let azero_rw_connection = if let Some(cid) = config.signer_cid {
            AzeroConnectionWithSigner::with_signer(
                azero::init(&config.azero_node_wss_url).await,
                cid,
                config.signer_port,
            )
            .expect("Cannot connect to signer")
        } else if config.dev {
            let azero_seed = "//".to_owned() + &config.dev_account_index.to_string();
            let keypair = aleph_client::keypair_from_string(&azero_seed);
            AzeroConnectionWithSigner::with_keypair(
                azero::init(&config.azero_node_wss_url).await,
                keypair,
            )
        } else {
            unimplemented!("Only dev mode is supported for now");
        };

        let azero_ro_connection = Arc::new(azero::init(&config.azero_node_wss_url).await);

        let wallet = if config.dev {
            // If no keystore path is provided, we use the default development mnemonic
            MnemonicBuilder::<English>::default()
                .phrase(DEV_MNEMONIC)
                .index(config.dev_account_index)
                .expect("Provided index is an integer between 0 and 9")
                .build()
                .expect("Mnemonic is correct")
        } else {
            assert!(
                !config.eth_keystore_path.is_empty(),
                "Keystore path must be provided unless relayer is run in dev mode"
            );

            LocalWallet::decrypt_keystore(&config.eth_keystore_path, &config.eth_keystore_password)
                .expect("Cannot decrypt eth wallet")
        };

        info!("Wallet address: {}", wallet.address());

        let eth_connection = Arc::new(
            eth::sign(eth::connect(&config.eth_node_http_url).await, wallet)
                .await
                .expect("Cannot sign the connection"),
        );

        debug!("Established connection to Ethereum node");

        let config_rc1 = Arc::clone(&config);
        let eth_connection_rc1 = Arc::clone(&eth_connection);
        let redis_connection_rc1 = Arc::clone(&redis_connection);

        info!("Starting Ethereum listener");

        tasks.push(tokio::spawn(async {
            EthListener::run(
                config_rc1,
                azero_rw_connection,
                eth_connection_rc1,
                redis_connection_rc1,
            )
            .await
            .expect("Ethereum listener task has failed")
        }));

        let config_rc2 = Arc::clone(&config);
        let azero_connection_rc2 = Arc::clone(&azero_ro_connection);
        let eth_connection_rc2 = Arc::clone(&eth_connection);
        let redis_connection_rc2 = Arc::clone(&redis_connection);

        info!("Starting AlephZero listener");

        tasks.push(tokio::spawn(async {
            AlephZeroListener::run(
                config_rc2,
                azero_connection_rc2,
                eth_connection_rc2,
                redis_connection_rc2,
            )
            .await
            .expect("AlephZero listener task has failed")
        }));

        for t in tasks {
            t.await.expect("task failure");
        }
    });

    process::exit(-1);
}
