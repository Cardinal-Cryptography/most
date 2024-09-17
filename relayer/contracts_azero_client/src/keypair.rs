use std::str::FromStr;

use anyhow::anyhow;
use subxt::{
    ext::sp_core::{ed25519, sr25519, Pair},
    PolkadotConfig,
};

use crate::{AccountId, AlephKeyPair, RawKeyPair};

type PairSigner = subxt::tx::PairSigner<PolkadotConfig, RawKeyPair>;

/// Used for signing extrinsic payload
pub struct KeyPair {
    inner: PairSigner,
}

impl Clone for KeyPair {
    fn clone(&self) -> Self {
        KeyPair::new(self.inner.signer().clone())
    }
}

impl FromStr for KeyPair {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> anyhow::Result<Self> {
        let pair = sr25519::Pair::from_string(s, None)
            .map_err(|e| anyhow!("Can't create pair from seed value: {:?}", e))?;
        Ok(KeyPair::new(pair))
    }
}

impl KeyPair {
    /// Constructs a new KeyPair from RawKeyPair
    pub fn new(keypair: RawKeyPair) -> Self {
        KeyPair {
            inner: PairSigner::new(keypair),
        }
    }

    /// Returns a reference to the inner RawKeyPair
    pub fn signer(&self) -> &RawKeyPair {
        self.inner.signer()
    }

    /// Returns corresponding AccountId
    pub fn account_id(&self) -> &AccountId {
        self.inner.account_id()
    }
}

/// Converts given seed phrase to a sr25519 [`KeyPair`] object.
/// * `seed` - a 12 or 24 word seed phrase
pub fn keypair_from_string(seed: &str) -> KeyPair {
    let pair = sr25519::Pair::from_string(seed, None).expect("Can't create pair from seed value");
    KeyPair::new(pair)
}

/// Converts given seed phrase to a sr25519 [`RawKeyPair`] object.
/// * `seed` - a 12 or 24 word seed phrase
pub fn raw_keypair_from_string(seed: &str) -> RawKeyPair {
    sr25519::Pair::from_string(seed, None).expect("Can't create pair from seed value")
}

/// Converts given seed phrase to a ed25519 [`AlephKeyPair`] object.
/// * `seed` - a 12 or 24 word seed phrase
pub fn aleph_keypair_from_string(seed: &str) -> AlephKeyPair {
    ed25519::Pair::from_string(seed, None).expect("Can't create pair from seed value")
}

/// Converts a key pair object to `AccountId`.
/// * `keypair` - a key-pair object, e.g. [`ed25519::Pair`] or [`sr25519::Pair`]
pub fn account_from_keypair<P>(keypair: &P) -> AccountId
where
    P: Pair,
    AccountId: From<<P as Pair>::Public>,
{
    AccountId::from(keypair.public())
}
