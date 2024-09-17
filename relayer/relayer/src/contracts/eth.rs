use contracts_azero_client::H160;
use ethers::{
    contract::{abigen, ContractError},
    middleware::Middleware,
    prelude::BlockNumber,
};

use crate::contracts::SignatureState;

abigen!(Most, "../eth/artifacts/contracts/Most.sol/Most.json");

pub async fn contract_signature_state<C: Middleware + 'static>(
    contract: &Most<C>,
    request_hash: [u8; 32],
    address: H160,
    committee_id: u128,
) -> Result<SignatureState, ContractError<C>> {
    use SignatureState::*;
    if !contract
        .needs_signature(request_hash, address, committee_id.into())
        .block(BlockNumber::Finalized)
        .await?
    {
        return Ok(Signed { finalized: true });
    }
    let is_signed = !contract
        .needs_signature(request_hash, address, committee_id.into())
        .block(BlockNumber::Latest)
        .await?;

    Ok(match is_signed {
        true => Signed { finalized: false }, // Signed but not yet finalized
        false => NeedSignature,              // Not signed and not finalized
    })
}
