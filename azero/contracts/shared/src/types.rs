use ink::env::hash::{HashOutput, Keccak256};

pub type Keccak256HashOutput = <Keccak256 as HashOutput>::Type;
