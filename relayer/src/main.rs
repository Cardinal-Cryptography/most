use std::{
    env, process,
    sync::{atomic::AtomicBool, Arc},
};

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
    connections::{azero, eth},
    listeners::{
        AdvisoryListener, AlephZeroListener, AzeroListenerError, EthListener, EthListenerError,
    },
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
        let emergency = Arc::new(AtomicBool::new(false));

        let mut tasks = Vec::with_capacity(4);

        let client = RedisClient::open(config.redis_node.clone())
            .expect("Cannot connect to the redis cluster instance");
        let redis_connection = Arc::new(Mutex::new(
            client
                .get_async_connection()
                .await
                .expect("Cannot make redis connection"),
        ));

        let azero_keypair = if config.dev {
            let azero_seed = "//".to_owned() + &config.dev_account_index.to_string();
            aleph_client::keypair_from_string(&azero_seed)
        } else {
            unimplemented!("Only dev mode is supported for now");
        };

        let azero_connection = Arc::new(azero::init(&config.azero_node_wss_url).await);
        let azero_signed_connection = Arc::new(azero::sign(&azero_connection, &azero_keypair));

        debug!("Established connection to Aleph Zero node");

        let config_rc1 = Arc::clone(&config);
        let emergency_rc1 = Arc::clone(&emergency);

        // run task only if address passed on CLI
        if config.advisory_contract_address.is_some() {
            tasks.push(tokio::spawn(async {
                AdvisoryListener::run(config_rc1, azero_connection, emergency_rc1)
                    .await
                    .expect("Advisory listener task has failed")
            }));
        }

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

        let config_rc2 = Arc::clone(&config);
        let azero_connection_rc1 = Arc::clone(&azero_signed_connection);
        let eth_connection_rc1 = Arc::clone(&eth_connection);
        let redis_connection_rc1 = Arc::clone(&redis_connection);
        let emergency_rc2 = Arc::clone(&emergency);

        info!("Starting Ethereum listener");

        tasks.push(tokio::spawn(async {
            EthListener::run(
                config_rc2,
                azero_connection_rc1,
                eth_connection_rc1,
                redis_connection_rc1,
                emergency_rc2,
            )
            .await
            .expect("Ethereum listener task has failed")
        }));

        let config_rc3 = Arc::clone(&config);
        let azero_connection_rc2 = Arc::clone(&azero_signed_connection);
        let eth_connection_rc2 = Arc::clone(&eth_connection);
        let redis_connection_rc2 = Arc::clone(&redis_connection);
        let emergency_rc3 = Arc::clone(&emergency);

        info!("Starting AlephZero listener");

        tasks.push(tokio::spawn(async {
            AlephZeroListener::run(
                config_rc3,
                azero_connection_rc2,
                eth_connection_rc2,
                redis_connection_rc2,
                emergency_rc3,
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
