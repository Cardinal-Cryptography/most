use crate::utils::*;
use assert2::assert;
use most::MostError;

use drink::session::Session;
use shared::hash_request_data;
const WAZERO_RATIO: u128 = 1_000_000;

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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let token = token::setup(&mut session, "TestToken".to_string(), most.into(), BOB);

    let wazero = wrapped_azero::setup(&mut session, BOB);
    let wazero_address: ink_primitives::AccountId = wazero.into();

    most::set_wazero(&mut session, &most, wazero_address, OWNER)
        .expect("Set wazero should succeed");

    let token_address: ink_primitives::AccountId = token.into();
    most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        false,
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let token = token::setup(&mut session, "TestToken".to_string(), bob(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    let result = most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        false,
        OWNER,
    );

    assert_eq!(result, Err(MostError::NoMintPermission()));
}

#[drink::test]
fn most_is_not_a_minter_for_native_psp22(mut session: Session) {
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let token = token::setup(&mut session, "TestToken".to_string(), bob(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    let result = most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        true,
        OWNER,
    );

    assert_eq!(result, Ok(()));
}

#[drink::test]
fn most_native_azero_transfer(mut session: Session) {
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let most_address: ink_primitives::AccountId = most.into();

    let wazero = wrapped_azero::setup(&mut session, BOB);
    let wazero_address: ink_primitives::AccountId = wazero.into();

    most::set_wazero(&mut session, &most, wazero_address, OWNER)
        .expect("Set wazero should succeed");

    most::add_pair(
        &mut session,
        &most,
        *wazero_address.as_ref(),
        REMOTE_TOKEN,
        true,
        OWNER,
    )
    .expect("Add pair should succeed");

    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");

    let amount_transferred = 1001;
    let base_fee = most::get_base_fee(&mut session, &most).expect("Get base fee should succeed");

    // Ensure that the sender has enough balance to cover the transfer
    session
        .sandbox()
        .mint_into(ALICE, 2 * base_fee + amount_transferred)
        .unwrap();

    let most_balance_before = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_before = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_before = wrapped_azero::total_supply(&mut session, &wazero);

    let result = most::send_request_native_azero(
        &mut session,
        &most,
        amount_transferred,
        REMOTE_RECEIVER,
        base_fee + amount_transferred,
        ALICE,
    );

    assert_eq!(result, Ok(()));

    let most_balance_after = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_after = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_after = wrapped_azero::total_supply(&mut session, &wazero);

    assert_eq!(most_balance_after, most_balance_before + amount_transferred);
    assert_eq!(alice_balance_after, alice_balance_before);
    assert_eq!(
        wazero_total_supply_after,
        wazero_total_supply_before + amount_transferred
    );
}

#[drink::test]
fn most_native_psp22_unlock(mut session: Session) {
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let most_address: ink_primitives::AccountId = most.into();

    let wazero = wrapped_azero::setup(&mut session, BOB);
    let wazero_address: ink_primitives::AccountId = wazero.into();

    most::set_wazero(&mut session, &most, wazero_address, OWNER)
        .expect("Set wazero should succeed");

    most::add_pair(
        &mut session,
        &most,
        *wazero_address.as_ref(),
        REMOTE_TOKEN,
        true,
        OWNER,
    )
    .expect("Add pair should succeed");

    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");

    let amount_transferred = 1001;
    let base_fee = most::get_base_fee(&mut session, &most).expect("Get base fee should succeed");

    // Ensure that the sender has enough balance to cover the transfer
    session
        .sandbox()
        .mint_into(ALICE, 2 * base_fee + amount_transferred)
        .unwrap();

    most::send_request_native_azero(
        &mut session,
        &most,
        amount_transferred,
        REMOTE_RECEIVER,
        base_fee + amount_transferred,
        ALICE,
    )
    .expect("Send request native should succeed");

    // Now bridge has wazero locked, so guardians can unlock by receive_request

    let committee_id: u128 = 0;
    let nonce: u128 = 1;

    let request_hash = hash_request_data(
        committee_id,
        wazero_address,
        amount_transferred,
        alice(),
        nonce,
    );

    let most_balance_before = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_before = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_before = wrapped_azero::total_supply(&mut session, &wazero);

    GUARDIANS
        .iter()
        .take(DEFAULT_THRESHOLD as usize)
        .for_each(|guardian| {
            let result = most::receive_request(
                &mut session,
                &most,
                request_hash,
                committee_id,
                *wazero_address.as_ref(),
                amount_transferred,
                *alice().as_ref(),
                nonce,
                guardian.clone(),
            );

            assert_eq!(result, Ok(()));
        });

    let most_balance_after = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_after = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_after = wrapped_azero::total_supply(&mut session, &wazero);

    assert_eq!(most_balance_after, most_balance_before - amount_transferred);
    assert_eq!(
        alice_balance_after,
        alice_balance_before + amount_transferred
    );
    assert_eq!(wazero_total_supply_after, wazero_total_supply_before);
}

#[drink::test]
fn most_native_azero_unlock(mut session: Session) {
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );

    let most_address: ink_primitives::AccountId = most.into();

    let wazero = wrapped_azero::setup(&mut session, BOB);
    let wazero_address: ink_primitives::AccountId = wazero.into();

    most::set_wazero(&mut session, &most, wazero_address, OWNER)
        .expect("Set wazero should succeed");

    most::add_pair(
        &mut session,
        &most,
        *wazero_address.as_ref(),
        REMOTE_TOKEN,
        true,
        OWNER,
    )
    .expect("Add pair should succeed");

    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");

    let amount_transferred = 1001;
    let base_fee = most::get_base_fee(&mut session, &most).expect("Get base fee should succeed");

    // Ensure that the sender has enough balance to cover the transfer
    session
        .sandbox()
        .mint_into(ALICE, 2 * base_fee + amount_transferred)
        .unwrap();

    most::send_request_native_azero(
        &mut session,
        &most,
        amount_transferred,
        REMOTE_RECEIVER,
        base_fee + amount_transferred,
        ALICE,
    )
    .expect("Send request native should succeed");

    // Now bridge has wazero locked, so guardians can unlock by receive_request

    let committee_id: u128 = 0;
    let nonce: u128 = 1;

    let request_hash = hash_request_data(
        committee_id,
        ZERO_ADDRESS.into(),
        amount_transferred * WAZERO_RATIO, // we multiply here since the decimals=18 on eth
        alice(),
        nonce,
    );

    let most_balance_before = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_before = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_before = wrapped_azero::total_supply(&mut session, &wazero);

    let alice_azero_balance_before = session.sandbox().free_balance(&ALICE);

    GUARDIANS
        .iter()
        .take(DEFAULT_THRESHOLD as usize)
        .for_each(|guardian| {
            let result = most::receive_request(
                &mut session,
                &most,
                request_hash,
                committee_id,
                ZERO_ADDRESS,
                amount_transferred * WAZERO_RATIO,
                *alice().as_ref(),
                nonce,
                guardian.clone(),
            );

            assert_eq!(result, Ok(()));
        });

    let most_balance_after = wrapped_azero::balance_of(&mut session, &wazero, most_address);
    let alice_balance_after = wrapped_azero::balance_of(&mut session, &wazero, alice());
    let wazero_total_supply_after = wrapped_azero::total_supply(&mut session, &wazero);

    let alice_azero_balance_after = session.sandbox().free_balance(&ALICE);

    assert_eq!(most_balance_after, most_balance_before - amount_transferred);
    assert_eq!(alice_balance_after, alice_balance_before);
    assert_eq!(
        wazero_total_supply_after,
        wazero_total_supply_before - amount_transferred
    );

    assert_eq!(
        alice_azero_balance_after,
        alice_azero_balance_before + amount_transferred
    );
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let token = token::setup(&mut session, "TestToken".to_string(), most.into(), BOB);

    let token_address: ink_primitives::AccountId = token.into();
    most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        false,
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
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
    session
        .sandbox()
        .set_timestamp(current_timestamp + 1000 * 60 * 60 * 24 * 2);

    assert_eq!(
        most::get_base_fee(&mut session, &most),
        Ok(DEFAULT_GAS_PRICE * RELAY_GAS_USAGE * 120 / 100)
    );
}

/// Reproduction of https://github.com/hats-finance/Most--Aleph-Zero-Bridge-0xab7c1d45ae21e7133574746b2985c58e0ae2e61d/issues/63
#[drink::test]
fn receive_request_after_switching_to_higher_threshold(mut session: Session) {
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
        GAS_ORACLE_MAX_AGE,
        ORACLE_CALL_GAS_LIMIT,
        BASE_FEE_BUFFER_PERCENTAGE,
        None,
        owner(),
        BOB,
        DEFAULT_ETH_TRANSFER_GAS_USAGE,
    );
    let token = token::setup(&mut session, "TestToken".to_string(), most.into(), BOB);

    let old_threshold = 4;
    most::set_halted(&mut session, &most, true, OWNER).expect("Halt should succeed");
    let token_address: ink_primitives::AccountId = token.into();
    most::add_pair(
        &mut session,
        &most,
        *token_address.as_ref(),
        REMOTE_TOKEN,
        false,
        OWNER,
    )
    .expect("Add pair should succeed");
    most::set_committee(
        &mut session,
        &most,
        guardian_accounts(),
        old_threshold,
        OWNER,
    )
    .expect("Set committee should succeed");
    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");

    let old_committee_id = most::get_current_committee_id(&mut session, &most)
        .expect("Get current committee id should succeed");
    let amount = 841189100000000;
    let receiver_address = alice();
    let request_nonce = 1;

    let request_hash = hash_request_data(
        old_committee_id,
        token_address,
        amount,
        receiver_address,
        request_nonce,
    );

    let new_threshold = 5;
    most::set_halted(&mut session, &most, true, OWNER).expect("Unhalt should succeed");
    most::set_committee(
        &mut session,
        &most,
        guardian_accounts(),
        new_threshold,
        OWNER,
    )
    .expect("Set committee should succeed");
    most::set_halted(&mut session, &most, false, OWNER).expect("Unhalt should succeed");

    let alice_balance_before = token::balance_of(&mut session, &token, alice());
    GUARDIANS
        .iter()
        .take(old_threshold as usize)
        .for_each(|guardian| {
            let result = most::receive_request(
                &mut session,
                &most,
                request_hash,
                old_committee_id,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
                guardian.clone(),
            );

            assert_eq!(result, Ok(()));
        });

    assert!(token::balance_of(&mut session, &token, alice()) == alice_balance_before + amount);
}
