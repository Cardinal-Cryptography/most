use crate::utils::*;

use drink::frame_support::sp_runtime::traits::Scale;
use drink::session::Session;
use ink_wrapper_types::Connection;

#[drink::test]
fn instantiate_token(mut session: Session) {
    let token = token::setup(&mut session, "TestToken".to_string(), alice(), BOB);
}

#[drink::test]
fn instantiate_most(mut session: Session) {
    let most = most::setup(
        &mut session,
        guardian_accounts(),
        DEFAULT_THRESHOLD,
        POCKET_MONEY,
        RELAY_GAS_USAGE,
        MIN_GAS_PRICE,
        MAX_GAS_PRICE,
        DEFAULT_GAS_PRICE,
        None,
        owner(),
        BOB,
    );
}

#[drink::test]
fn instantiate_gas_oracle(mut session: Session) {
    let gas_price_oracle = gas_price_oracle::setup(
        &mut session,
        owner(),
        2 * MIN_GAS_PRICE,
        BOB,
    );
}

/*#[drink::test]
fn add_liquidity(mut session: Session) {
    upload_all(&mut session);

    let fee_to_setter = bob();

    let factory = factory::setup(&mut session, fee_to_setter);
    let ice = psp22::setup(&mut session, ICE.to_string(), BOB);
    let wazero = wazero::setup(&mut session);
    let router = router::setup(&mut session, factory.into(), wazero.into());

    let token_amount = 10_000;
    psp22::increase_allowance(&mut session, ice.into(), router.into(), token_amount, BOB).unwrap();

    let all_pairs_length_before = session
        .query(factory.all_pairs_length())
        .unwrap()
        .result
        .unwrap();

    let now = get_timestamp(&mut session);
    set_timestamp(&mut session, now);
    let deadline = now + 10;

    let (amount_ice, amount_native, liquidity_minted) = session
        .execute(
            router
                .add_liquidity_native(
                    ice.into(),
                    token_amount,
                    token_amount,
                    token_amount,
                    bob(),
                    deadline,
                )
                .with_value(token_amount),
        )
        .unwrap()
        .result
        .unwrap()
        .unwrap();

    let ice_wazero_pair: pair_contract::Instance = session
        .query(factory.get_pair(ice.into(), wazero.into()))
        .unwrap()
        .result
        .unwrap()
        .unwrap()
        .into();

    let minimum_liquidity = session
        .query(ice_wazero_pair.get_minimum_liquidity())
        .unwrap()
        .result
        .unwrap();

    let all_pairs_length_after = session
        .query(factory.all_pairs_length())
        .unwrap()
        .result
        .unwrap();

    assert!(
        all_pairs_length_before + 1 == all_pairs_length_after,
        "There should be one more pair"
    );
    assert!(amount_ice == token_amount,);
    assert!(amount_native == token_amount,);
    // Matches the formula from the whitepaper for minting liquidity tokens for a newly created pair.
    assert!(
        liquidity_minted == token_amount.mul(token_amount).integer_sqrt() - minimum_liquidity,
        "Should mint expected amount of LP tokens"
    );
}*/
