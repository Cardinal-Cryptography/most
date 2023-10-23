#![cfg_attr(not(feature = "std"), no_std, no_main)]

mod hash;
mod helpers;
mod types;

pub use hash::keccak256;
pub use helpers::concat_u8_arrays;
pub use types::{CallInput, Keccak256HashOutput, Selector};
