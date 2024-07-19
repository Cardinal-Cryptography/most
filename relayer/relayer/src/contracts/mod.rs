mod azero;
#[cfg(not(feature = "l2"))]
mod eth;
#[cfg(feature = "l2")]
mod l2_eth;

pub use azero::*;
#[cfg(not(feature = "l2"))]
pub use eth::*;
#[cfg(feature = "l2")]
pub use l2_eth::*;

pub enum SignatureState {
    Signed { finalized: bool },
    NeedSignature,
}
