#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod hash;
mod types;

pub use hash::keccak256;
pub use types::{CallInput, Keccak256HashOutput, Selector};
