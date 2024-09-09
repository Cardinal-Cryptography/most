pub const ALEPH_BLOCK_PROD_TIME_SEC: u64 = 1;

#[cfg(not(feature = "l2"))]
pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 12;

#[cfg(feature = "l2")]
pub const ETH_BLOCK_PROD_TIME_SEC: u64 = 1;
