use std::{future::Future, ops::AddAssign};

use anyhow::{anyhow, Error, Result};
use ethers::types::U256;
use log::info;
use tokio::time::{sleep, Duration};

use crate::client::Balance;

pub async fn wait_for_balance_change<F, R>(
    get_current_balance: F,
    target_balance: Balance,
    max_eth_fee: Option<U256>,
    max_azero_fee: Option<u128>,
    wait_max_minutes: u64,
) -> Result<()>
where
    F: Fn() -> R,
    R: Future<Output = Result<Balance, Error>> + Sized,
{
    let tick = Duration::from_secs(12_u64);
    let wait_max = Duration::from_secs(60_u64 * wait_max_minutes);

    info!(
        "Waiting a max. of {:?} minutes for token transfer to be detected...",
        wait_max_minutes
    );

    let mut wait = Duration::from_secs(0_u64);

    while wait <= wait_max {
        sleep(tick).await;
        wait.add_assign(tick);
        let current_balance = get_current_balance().await?;
        info!("Current balance: {:?}", current_balance);
        if current_balance.satisfies_target(&target_balance, max_eth_fee, max_azero_fee) {
            info!("Required balance change detected");
            return Ok(());
        }
        if wait.as_secs() % 60 == 0 {
            info!("minutes elapsed: {:?}", wait.as_secs() / 60)
        }
    }

    Err(anyhow!("Failed to detect required balance change.",))
}
