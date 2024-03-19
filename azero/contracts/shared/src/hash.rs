use ink::env::{hash::Keccak256, hash_bytes};
use ink::primitives::AccountId;

use crate::Keccak256HashOutput;

pub fn keccak256(input: &[u8]) -> Keccak256HashOutput {
    let mut output = Keccak256HashOutput::default();
    hash_bytes::<Keccak256>(input, &mut output);
    output
}

pub fn hash_request_data(
    commitee_id: u128,
    token_address: AccountId,
    amount: u128,
    receiver_address: AccountId,
    request_nonce: u128,
) -> Keccak256HashOutput {
    let request_data = [
        &commitee_id.to_le_bytes(),
        AsRef::<[u8]>::as_ref(&token_address),
        &amount.to_le_bytes(),
        AsRef::<[u8]>::as_ref(&receiver_address),
        &request_nonce.to_le_bytes(),
    ]
    .concat();
    keccak256(&request_data)
}
