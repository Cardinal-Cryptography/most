use std::{future::Future, ops::AddAssign};

use anyhow::{anyhow, Error, Result};
use log::info;
use tokio::time::{sleep, Duration};

pub async fn wait_for_balance_change<F, R>(
    transfer_amount: u128,
    balance_pre_transfer: u128,
    get_current_balance: F,
    wait_max_minutes: u64,
) -> Result<()>
where
    F: Fn() -> R,
    R: Future<Output = Result<u128, Error>> + Sized,
{
    let tick = Duration::from_secs(12_u64);
    let wait_max = Duration::from_secs(60_u64 * wait_max_minutes);

    info!(
        "Waiting a max. of {:?} minutes for finalization",
        wait_max_minutes
    );

    let mut wait = Duration::from_secs(0_u64);

    while wait <= wait_max {
        sleep(tick).await;
        wait.add_assign(tick);

        let balance_current = get_current_balance().await?;
        let balance_change = balance_current - balance_pre_transfer;
        if balance_change == transfer_amount {
            info!("Required balance change detected: {:?}", balance_change);
            return Ok(());
        }
        if wait.as_secs() % 60 == 0 {
            info!("minutes elapsed: {:?}", wait.as_secs() / 60)
        }
    }

    Err(anyhow!(
        "Failed to detect required balance change of {:?}",
        transfer_amount
    ))
}
