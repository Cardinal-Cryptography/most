use config::{Config, Load};
use eyre::Result;
use log::info;
use std::env;
use std::sync::Arc;

mod azero_listener;
mod config;
mod eth_listener;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::load());

    env::set_var("RUST_LOG", &config.log_level);
    env_logger::init();

    info!("{:#?}", &config);

    eth_listener::run(config).await?;

    Ok(())
}
