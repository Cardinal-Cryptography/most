use crate::utils::*;
use most::MostError;

use drink::session::Session;
use shared::hash_request_data;

#[drink::test]
fn no_duplicate_guardians_allowed(mut session: Session) {
    let mut guardians = guardian_accounts();

    let most = most::setup(
        &mut session,
        guardians.clone(),
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

    guardians.push(guardians[0]);
    let result = most::set_committee(&mut session, &most, guardians, DEFAULT_THRESHOLD, OWNER);
    assert_eq!(result, Err(MostError::DuplicateCommitteeMember()));
}

#[drink::test]
fn no_zero_amount_allowed(mut session: Session) {
    mint_to_default_accounts(&mut session);

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
    let token = token::setup(&mut session, "TestToken".to_string(), most.into(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        OWNER,
    )
    .expect("Add pair should succeed");

    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");
    token::increase_allowance(&mut session, &token, most.into(), 1000, BOB)
        .expect("Increase allowance should succeed");
    let result = most::send_request(
        &mut session,
        &most,
        *token_address.as_ref(),
        0,
        REMOTE_RECEIVER,
        DEFAULT_GAS_PRICE * RELAY_GAS_USAGE,
        BOB,
    );

    assert_eq!(result, Err(MostError::ZeroTransferAmount()));
}

#[drink::test]
fn most_needs_to_be_token_minter_to_add_pair(mut session: Session) {
    mint_to_default_accounts(&mut session);

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
    let token = token::setup(&mut session, "TestToken".to_string(), bob(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    let result = most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        OWNER,
    );

    assert_eq!(result, Err(MostError::NoMintPermission()));
}

#[drink::test]
fn correct_receive_request(mut session: Session) {
    mint_to_default_accounts(&mut session);

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
    let token = token::setup(&mut session, "TestToken".to_string(), most.into(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        OWNER,
    )
    .expect("Add pair should succeed");

    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");
    token::transfer(&mut session, &token, most.into(), 1000, BOB).expect("Transfer should succeed");

    let alice_balance_before = token::balance_of(&mut session, &token, alice());

    let committee_id: u128 = 0;
    let amount: u128 = 100;
    let nonce: u128 = 1;

    let request_hash = hash_request_data(committee_id, token_address, amount, alice(), nonce);

    GUARDIANS
        .iter()
        .take(DEFAULT_THRESHOLD as usize)
        .for_each(|guardian| {
            let result = most::receive_request(
                &mut session,
                &most,
                request_hash,
                committee_id,
                *token_address.as_ref(),
                amount,
                *alice().as_ref(),
                nonce,
                guardian.clone(),
            );

            //println!("Guardian: {:?}", guardian);
            assert_eq!(result, Ok(()));
        });

    assert_eq!(
        token::balance_of(&mut session, &token, alice()),
        alice_balance_before + 100
    );
}

#[drink::test]
fn outdated_oracle_price(mut session: Session) {
    mint_to_default_accounts(&mut session);

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

    let oracle = gas_price_oracle::setup(&mut session, alice(), 2 * MIN_GAS_PRICE, BOB);

    most::set_gas_price_oracle(&mut session, &most, oracle.into(), OWNER)
        .expect("Set gas price oracle should succeed");

    assert_eq!(
        most::get_base_fee(&mut session, &most),
        Ok(2 * MIN_GAS_PRICE * RELAY_GAS_USAGE * 120 / 100)
    );

    let current_timestamp = session.sandbox().get_timestamp();
    // Advance the timestamp by 2 dayss
    session.sandbox().set_timestamp(current_timestamp + 1000 * 60 * 60 * 24 * 2);

    assert_eq!(
        most::get_base_fee(&mut session, &most),
        Ok(DEFAULT_GAS_PRICE * RELAY_GAS_USAGE * 120 / 100)
    );
}
