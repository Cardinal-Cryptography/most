use crate::*;

use anyhow::Result;
use drink::AccountId32;
use ink_primitives::AccountId;
use ink_wrapper_types::{Connection, ContractResult, InkLangError, ToAccountId};

type Session = drink::session::Session<drink::runtime::MinimalRuntime>;

pub const ALICE: drink::AccountId32 = AccountId32::new([0u8; 32]);
pub const BOB: drink::AccountId32 = AccountId32::new([1u8; 32]);
pub const OWNER: drink::AccountId32 = AccountId32::new([3u8; 32]);

pub const GUARDIANS: [drink::AccountId32; 8] = [
    AccountId32::new([10u8; 32]),
    AccountId32::new([11u8; 32]),
    AccountId32::new([12u8; 32]),
    AccountId32::new([13u8; 32]),
    AccountId32::new([14u8; 32]),
    AccountId32::new([15u8; 32]),
    AccountId32::new([16u8; 32]),
    AccountId32::new([17u8; 32]),
];

pub fn alice() -> ink_primitives::AccountId {
    AsRef::<[u8; 32]>::as_ref(&ALICE).clone().into()
}

pub fn bob() -> ink_primitives::AccountId {
    AsRef::<[u8; 32]>::as_ref(&BOB).clone().into()
}

pub fn owner() -> ink_primitives::AccountId {
    AsRef::<[u8; 32]>::as_ref(&OWNER).clone().into()
}

pub fn guardian_accounts() -> Vec<ink_primitives::AccountId> {
    GUARDIANS
        .iter()
        .map(|x| AsRef::<[u8; 32]>::as_ref(x).clone().into())
        .collect()
}

pub const DEFAULT_THRESHOLD: u128 = 5;
pub const REMOTE_TOKEN: [u8; 32] = [0x1; 32];
pub const REMOTE_RECEIVER: [u8; 32] = [0x2; 32];

pub const APPROX_GWEI_PRICE: u128 = 3000000;
pub const MIN_GAS_PRICE: u128 = 20 * APPROX_GWEI_PRICE;
pub const MAX_GAS_PRICE: u128 = 150 * APPROX_GWEI_PRICE;
pub const DEFAULT_GAS_PRICE: u128 = 60 * APPROX_GWEI_PRICE;
pub const POCKET_MONEY: u128 = 1000000000000;
pub const RELAY_GAS_USAGE: u128 = 450000;

pub mod most {
    use super::*;
    use wrappers::most::{self, Instance as Most};

    pub fn setup(
        session: &mut Session,
        committee: Vec<AccountId>,
        signature_threshold: u128,
        pocket_money: u128,
        relay_gas_usage: u128,
        min_gas_price: u128,
        max_gas_price: u128,
        default_gas_price: u128,
        gas_price_oracle: Option<AccountId>,
        owner: AccountId,
        caller: drink::AccountId32,
    ) -> Most {
        let _code_hash = session.upload_code(most::upload()).unwrap();

        let _ = session.set_actor(caller);

        let instance = Most::new(
            committee,
            signature_threshold,
            pocket_money,
            relay_gas_usage,
            min_gas_price,
            max_gas_price,
            default_gas_price,
            gas_price_oracle,
            owner,
        );

        session
            .instantiate(instance)
            .unwrap()
            .result
            .to_account_id()
            .into()
    }
}

pub mod token {
    use super::*;
    use token::Instance as Token;
    use wrappers::token::{self, PSP22};

    pub fn setup(
        session: &mut Session,
        name: String,
        minter_burner: AccountId,
        caller: drink::AccountId32,
    ) -> Token {
        let _code_hash = session.upload_code(token::upload()).unwrap();

        let _ = session.set_actor(caller);

        let instance = Token::new(
            1_000_000_000u128 * 10u128.pow(18),
            Some(name.clone()),
            Some(name),
            18,
            minter_burner,
        );

        session
            .instantiate(instance)
            .unwrap()
            .result
            .to_account_id()
            .into()
    }

    /// Increases allowance of given token to given spender by given amount.
    pub fn increase_allowance(
        session: &mut Session,
        token: &Token,
        spender: AccountId,
        amount: u128,
        caller: drink::AccountId32,
    ) -> Result<(), token::PSP22Error> {
        let _ = session.set_actor(caller);

        handle_ink_error(
            session
                .execute(PSP22::increase_allowance(token, spender, amount))
                .unwrap(),
        )
    }

    /// Returns balance of given token for given account.
    /// Fails if anything other than success.
    pub fn balance_of(session: &mut Session, token: &Token, account: AccountId) -> u128 {
        handle_ink_error(session.query(PSP22::balance_of(token, account)).unwrap())
    }

    pub fn total_supply(session: &mut Session, token: &Token) -> u128 {
        handle_ink_error(session.query(PSP22::total_supply(token)).unwrap())
    }
}

pub mod gas_price_oracle {
    use super::*;
    use wrappers::gas_price_oracle::{self, Instance as GasPriceOracle};

    pub fn setup(
        session: &mut Session,
        owner: AccountId,
        init_price: u128,
        caller: drink::AccountId32,
    ) -> GasPriceOracle {
        let _code_hash = session.upload_code(gas_price_oracle::upload()).unwrap();

        let _ = session.set_actor(caller);

        let instance = GasPriceOracle::new(
            owner,
            init_price,
        );

        session
            .instantiate(instance)
            .unwrap()
            .result
            .to_account_id()
            .into()
    }
}

pub fn get_timestamp(session: &mut Session) -> u64 {
    session.sandbox().get_timestamp()
}

pub fn set_timestamp(session: &mut Session, timestamp: u64) {
    session.sandbox().set_timestamp(timestamp);
}

pub fn handle_ink_error<R>(res: ContractResult<Result<R, InkLangError>>) -> R {
    match res.result {
        Err(ink_lang_err) => panic!("InkLangError: {:?}", ink_lang_err),
        Ok(r) => r,
    }
}
