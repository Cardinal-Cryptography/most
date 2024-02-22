use std::{future::Future, ops::AddAssign};

use log::info;

pub async fn wait_for_balance_change<F, R>(
    transfer_amount: u128,
    balance_pre_transfer: u128,
    get_current_balance: F,
    wait_max_minutes: u64,
) -> anyhow::Result<()>
where
    F: Fn() -> R,
    R: Future<Output = Result<u128, anyhow::Error>> + Sized,
{
    let tick = tokio::time::Duration::from_secs(30_u64);
    let wait_max = tokio::time::Duration::from_secs(60_u64 * wait_max_minutes);

    info!(
        "Waiting a max. of {:?} minutes for finalization",
        wait_max_minutes
    );

    let mut wait = tokio::time::Duration::from_secs(0_u64);

    while wait <= wait_max {
        tokio::time::sleep(tick).await;
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

    Err(anyhow::anyhow!(
        "Failed to detect required balance change of {:?}",
        transfer_amount
    ))
}
