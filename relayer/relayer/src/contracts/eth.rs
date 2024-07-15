use aleph_client::sp_core::H160;
use ethers::{
    contract::{abigen, ContractError},
    middleware::Middleware,
    prelude::BlockNumber,
};

abigen!(Most, "../eth/artifacts/contracts/Most.sol/Most.json");
abigen!(WETH9, "../eth/artifacts/contracts/WETH9.sol/WETH9.json");

pub enum SignatureState {
    #[cfg_attr(feature = "evm", allow(dead_code))]
    SignedNotFinalized,
    NeedSignature,
    Finalized,
}

#[cfg(feature = "evm")]
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
        Ok(Finalized)
    } else {
        Ok(NeedSignature)
    }
}

#[cfg(not(feature = "evm"))]
pub async fn contract_signature_state<C: Middleware + 'static>(
    contract: &Most<C>,
    request_hash: [u8; 32],
    address: H160,
    committee_id: u128,
) -> Result<SignatureState, ContractError<C>> {
    use SignatureState::*;
    let is_finalized = !contract
        .needs_signature(request_hash, address, committee_id.into())
        .block(BlockNumber::Finalized)
        .await?;
    let is_signed = !contract
        .needs_signature(request_hash, address, committee_id.into())
        .block(BlockNumber::Latest)
        .await?;

    Ok(match (is_signed, is_finalized) {
        (_, true) => Finalized,          // Does not need signature on finalized block
        (true, _) => SignedNotFinalized, // Signed but not yet finalized
        (false, _) => NeedSignature,     // Not signed and not finalized
    })
}
