use ink::env::{hash::Keccak256, hash_bytes};

use crate::Keccak256HashOutput;

pub fn keccak256(input: &[u8]) -> Keccak256HashOutput {
    let mut output = Keccak256HashOutput::default();
    hash_bytes::<Keccak256>(input, &mut output);
    output
}
