use config::{Config, Load};
use log::info;
use std::env;
use std::sync::Arc;
use tokio::runtime::Runtime;

mod config;
mod eth_contract_listener;

#[tokio::main]
async fn main() {
    let config = Arc::new(Config::load());

    env::set_var("RUST_LOG", &config.log_level);
    env_logger::init();

    info!("{:#?}", &config);

    eth_contract_listener::run(config)
        .await
        .expect("Contract listener task failed");

    // let runtime = Runtime::new().unwrap();

    // runtime.block_on(async {
    //     let mut tasks = Vec::with_capacity(3);

    //     let config_rc1 = Arc::clone(&config);
    //     tasks.push(tokio::spawn(async {
    //         eth_contract_listener::run(config_rc1).await;
    //     }));

    //     // let config_rc2 = Arc::clone(&config);
    //     // tasks.push (tokio::spawn(async {
    //     //     command_processor::run (config_rc2).await;
    //     // }));

    //     // let config_rc3 = Arc::clone(&config);
    //     // let db_rc2 = Arc::clone (&db);
    //     // tasks.push (tokio::spawn(async {
    //     //     materialized_view::run (config_rc3, db_rc2).await;
    //     // }));

    //     for t in tasks {
    //         t.await.expect("Ooops!");
    //     }
    // });
}
