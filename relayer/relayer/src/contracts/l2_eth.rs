use aleph_client::sp_core::H160;
use ethers::{
    contract::{abigen, ContractError},
    middleware::Middleware,
    prelude::BlockNumber,
};

use crate::contracts::SignatureState;

abigen!(Most, "../eth/artifacts/contracts/MostL2.sol/MostL2.json");

pub async fn contract_signature_state<C: Middleware + 'static>(
    contract: &Most<C>,
    request_hash: [u8; 32],
    address: H160,
    committee_id: u128,
) -> Result<SignatureState, ContractError<C>> {
    use SignatureState::*;
    if !contract
        .needs_signature(request_hash, address, committee_id.into())
        .block(BlockNumber::Latest)
        .await?
    {
        Ok(Signed { finalized: true })
    } else {
        Ok(NeedSignature)
    }
}
