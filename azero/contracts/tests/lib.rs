#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(all(test, feature = "e2e-tests"))]
mod events;

#[cfg(all(test, feature = "e2e-tests"))]
mod e2e {
    use core::fmt::Debug;

    use ink::{
        codegen::TraitCallBuilder,
        env::{
            call::{
                utils::{ReturnType, Set},
                Call, CallBuilder, ExecutionInput, FromAccountId,
            },
            DefaultEnvironment,
        },
        primitives::AccountId,
    };
    use ink_e2e::{
        account_id, alice, bob, build_message, charlie, dave, eve, ferdie, subxt::dynamic::Value,
        AccountKeyring, Keypair, PolkadotConfig,
    };
    use most::{
        most::{CrosschainTransferRequest, RequestProcessed, RequestSigned},
        MostError, MostRef,
    };
    use oracle::oracle::OracleRef;
    use psp22::{PSP22Error, PSP22};
    use scale::{Decode, Encode};
    use shared::{keccak256, Keccak256HashOutput};
    use wrapped_token::TokenRef;

    use crate::events::{
        filter_decode_events_as, get_contract_emitted_events, ContractEmitted, EventWithTopics,
    };

    type CommitteeId = u128;

    const TOKEN_INITIAL_SUPPLY: u128 = 10000;
    const DEFAULT_THRESHOLD: u128 = 3;
    const DECIMALS: u8 = 8;
    const REMOTE_TOKEN: [u8; 32] = [0x1; 32];
    const REMOTE_RECEIVER: [u8; 32] = [0x2; 32];

    const MIN_FEE: u128 = 10000000000000;
    const MAX_FEE: u128 = 100000000000000;
    const DEFAULT_FEE: u128 = 30000000000000;
    const DEFAULT_POCKET_MONEY: u128 = 1000000000000;
    const DEFAULT_RELAY_GAS_USAGE: u128 = 50000;

    const DEFAULT_COMMITTEE_ID: CommitteeId = 0;

    #[ink_e2e::test]
    fn simple_deploy_works(mut client: ink_e2e::Client<C, E>) {
        let _most_address = instantiate_most(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            DEFAULT_POCKET_MONEY,
            DEFAULT_RELAY_GAS_USAGE,
            MIN_FEE,
            MAX_FEE,
            DEFAULT_FEE,
        )
        .await;
    }

    #[ink_e2e::test]
    fn owner_can_add_a_new_pair(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let add_pair_res = most_add_pair(
            &mut client,
            &alice(),
            most_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await;

        assert!(add_pair_res.is_ok());
    }

    #[ink_e2e::test]
    fn non_owner_cannot_add_a_new_pair(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let add_pair_res = most_add_pair(
            &mut client,
            &bob(),
            most_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await;

        assert_eq!(
            add_pair_res.expect_err("Bob should not be able to add a pair as he is not the owner"),
            MostError::NotOwner(account_id(AccountKeyring::Bob))
        );
    }

    #[ink_e2e::test]
    fn send_request_burns_tokens(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        let total_supply_before = psp22_total_supply(&mut client, token_address)
            .await
            .expect("total supply before");

        let balance_before = psp22_balance_of(
            &mut client,
            token_address,
            account_id(AccountKeyring::Alice),
        )
        .await
        .expect("balance_of should succeed");

        let amount_to_send = 1000;

        psp22_approve(
            &mut client,
            &alice(),
            token_address,
            most_address,
            amount_to_send,
        )
        .await
        .expect("approval should succeed");

        most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await
        .expect("send request should succeed");

        let balance_after = psp22_balance_of(
            &mut client,
            token_address,
            account_id(AccountKeyring::Alice),
        )
        .await
        .expect("balance before");

        assert_eq!(
            balance_after,
            balance_before - amount_to_send,
            "sender balance after should be lowered by that amount"
        );

        let total_supply_after = psp22_total_supply(&mut client, token_address)
            .await
            .expect("total supply before");

        assert_eq!(
            total_supply_after,
            total_supply_before - amount_to_send,
            "total supply after should be lowered by the sent amount"
        );
    }

    #[ink_e2e::test]
    fn send_request_fails_on_non_whitelisted_token(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let amount_to_send = 1000;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("base fee");

        let send_request_res = most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        assert_eq!(
            send_request_res.expect_err("Request should fail for a non-whitelisted token"),
            MostError::UnsupportedPair
        );
    }

    #[ink_e2e::test]
    fn correct_request(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let amount_to_send = 1000;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        psp22_approve(
            &mut client,
            &alice(),
            token_address,
            most_address,
            amount_to_send,
        )
        .await
        .expect("approval should succeed");

        let send_request_res = most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        match send_request_res {
            Ok(call_res) => {
                // 1 PSP22Event::Transfer event for burn and 1 `CrosschainTransferRequest`
                assert_eq!(call_res.events.len(), 2);

                let request_events =
                    filter_decode_events_as::<CrosschainTransferRequest>(call_res.events);

                // `CrosschainTransferRequest` event
                assert_eq!(request_events.len(), 1);
                assert_eq!(
                    request_events[0],
                    CrosschainTransferRequest {
                        committee_id: 0,
                        dest_token_address: REMOTE_TOKEN,
                        amount: amount_to_send,
                        dest_receiver_address: REMOTE_RECEIVER,
                        request_nonce: 0,
                    }
                );
            }
            Err(e) => panic!("Request should succeed: {:?}", e),
        }
    }

    #[ink_e2e::test]
    fn receive_request_can_only_be_called_by_guardians(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash = hash_request_data(
            DEFAULT_COMMITTEE_ID,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        let alice_receive_request_res = most_receive_request(
            &mut client,
            &alice(),
            most_address,
            request_hash,
            DEFAULT_COMMITTEE_ID,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        assert_eq!(
            alice_receive_request_res.expect_err("Receive request should fail for non-guardians"),
            MostError::NotInCommittee
        );
    }

    #[ink_e2e::test]
    fn receive_request_non_matching_hash(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let incorrect_hash = [0x3; 32];
        let receive_request_res = most_receive_request(
            &mut client,
            &bob(),
            most_address,
            incorrect_hash,
            DEFAULT_COMMITTEE_ID,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        assert_eq!(
            receive_request_res.expect_err("Receive request should fail for non-matching hash"),
            MostError::HashDoesNotMatchData
        );
    }

    #[ink_e2e::test]
    fn receive_request_executes_request_after_enough_confirmations(
        mut client: ink_e2e::Client<C, E>,
    ) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let amount = 841189100000000;

        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash = hash_request_data(
            DEFAULT_COMMITTEE_ID,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        for i in 0..(DEFAULT_THRESHOLD as usize) {
            let signer = &guardian_keys()[i];
            let receive_res = most_receive_request(
                &mut client,
                signer,
                most_address,
                request_hash,
                DEFAULT_COMMITTEE_ID,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await;

            match receive_res {
                Ok(call_res) => {
                    let events = call_res.events;
                    if i == (DEFAULT_THRESHOLD - 1) as usize {
                        assert_eq!(events.len(), 3);
                        assert_eq!(
                            filter_decode_events_as::<RequestProcessed>(vec![events[2].clone()])[0],
                            RequestProcessed {
                                request_hash,
                                dest_token_address: *token_address.as_ref(),
                            }
                        );
                    } else {
                        assert_eq!(events.len(), 1);
                        assert_eq!(filter_decode_events_as::<RequestSigned>(events).len(), 1);
                    }
                }
                Err(e) => panic!("Receive request should succeed: {:?}", e),
            }
        }

        let balance = psp22_balance_of(&mut client, token_address, receiver_address)
            .await
            .expect("balance before");

        assert_eq!(balance, amount);
    }

    #[ink_e2e::test]
    fn receive_request_not_enough_signatures(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash = hash_request_data(
            DEFAULT_COMMITTEE_ID,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        for i in 0..(DEFAULT_THRESHOLD - 1) as usize {
            let signer = &guardian_keys()[i];
            let receive_res = most_receive_request(
                &mut client,
                signer,
                most_address,
                request_hash,
                DEFAULT_COMMITTEE_ID,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await;

            match receive_res {
                Ok(call_res) => {
                    assert_eq!(call_res.events.len(), 1);
                    assert_eq!(
                        filter_decode_events_as::<RequestSigned>(call_res.events)[0],
                        RequestSigned {
                            signer: guardian_ids()[i],
                            request_hash,
                        }
                    );
                }
                Err(e) => panic!("Receive request should succeed: {:?}", e),
            }
        }

        let balance_of_call = build_message::<TokenRef>(token_address)
            .call(|token| token.balance_of(receiver_address));
        let balance = client
            .call_dry_run(&alice(), &balance_of_call, 0, None)
            .await
            .return_value();

        assert_eq!(balance, 0);
    }

    #[ink_e2e::test]
    fn base_fee_too_low(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        let amount = 841189100000000;

        let send_request_res = most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount,
            REMOTE_RECEIVER,
            base_fee - 1,
        )
        .await;

        assert_eq!(
            send_request_res.expect_err("Request should fail without allowance"),
            MostError::BaseFeeTooLow
        );
    }

    #[ink_e2e::test]
    fn pocket_money(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, false).await;

        // seed contract with some funds for pocket money transfers
        let call_data = vec![
            Value::unnamed_variant("Id", [Value::from_bytes(most_address)]),
            Value::u128(10 * DEFAULT_POCKET_MONEY),
        ];

        client
            .runtime_call(&alice(), "Balances", "transfer", call_data)
            .await
            .expect("runtime call failed");

        let amount = 841189100000000;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash = hash_request_data(
            DEFAULT_COMMITTEE_ID,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        let azero_balance_before = client
            .balance(receiver_address)
            .await
            .expect("native balance before");

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            most_receive_request(
                &mut client,
                signer,
                most_address,
                request_hash,
                DEFAULT_COMMITTEE_ID,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await
            .expect("Receive request should succeed");
        }

        let azero_balance_after = client
            .balance(receiver_address)
            .await
            .expect("native balance after");

        assert_eq!(
            azero_balance_after,
            azero_balance_before + DEFAULT_POCKET_MONEY
        );
    }

    #[ink_e2e::test]
    fn pocket_money_wont_pay_from_rewards(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let amount_to_send = 1000;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        psp22_approve(
            &mut client,
            &alice(),
            token_address,
            most_address,
            amount_to_send,
        )
        .await
        .expect("approval should succeed");

        // Send request so there are rewards in the contract
        most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await
        .expect("Request should succeed");

        let amount = 841189100000000;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash = hash_request_data(
            DEFAULT_COMMITTEE_ID,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        let azero_balance_before = client
            .balance(receiver_address)
            .await
            .expect("native balance before");

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            most_receive_request(
                &mut client,
                signer,
                most_address,
                request_hash,
                DEFAULT_COMMITTEE_ID,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await
            .expect("Receive request should succeed");
        }

        let azero_balance_after = client
            .balance(receiver_address)
            .await
            .expect("native balance after");

        assert_eq!(azero_balance_after, azero_balance_before);
    }

    #[ink_e2e::test]
    fn committee_rewards(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let amount = 1000;
        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        psp22_approve(&mut client, &alice(), token_address, most_address, amount)
            .await
            .expect("approval should succeed");

        most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await
        .expect("send request should succeed");

        let committee_id = most_committee_id(&mut client, most_address)
            .await
            .expect("committe id");

        let total_rewards = most_committee_rewards(&mut client, most_address, committee_id)
            .await
            .expect("committee rewards");

        assert_eq!(total_rewards, base_fee);

        let committee_size = guardian_ids().len();
        for i in 0..committee_size {
            let member_id = guardian_ids()[i];

            let guardian_balance_before = client
                .balance(member_id)
                .await
                .expect("guardian balance before");

            most_request_payout(&mut client, &alice(), most_address, committee_id, member_id)
                .await
                .expect("request payout");

            let guardian_balance_after = client
                .balance(member_id)
                .await
                .expect("guardian balance after");

            assert_eq!(
                guardian_balance_after,
                guardian_balance_before + (total_rewards / committee_size as u128)
            );
        }

        // no double spend is possible
        let bob_balance_before = client
            .balance(account_id(AccountKeyring::Bob))
            .await
            .expect("signer balance before");

        most_request_payout(
            &mut client,
            &alice(),
            most_address,
            committee_id,
            account_id(AccountKeyring::Bob),
        )
        .await
        .expect("request payout twice results in a no-op");

        let bob_balance_after = client
            .balance(account_id(AccountKeyring::Bob))
            .await
            .expect("signer balance after");

        assert_eq!(bob_balance_after, bob_balance_before);
    }

    #[ink_e2e::test]
    fn past_committee_rewards(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        let amount = 1000;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        psp22_approve(&mut client, &alice(), token_address, most_address, amount)
            .await
            .expect("approval should succeed");

        most_send_request(
            &mut client,
            &alice(),
            most_address,
            token_address,
            amount,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await
        .expect("send request should succeed");

        let previous_committee_id = most_committee_id(&mut client, most_address)
            .await
            .expect("committe id");

        let previous_committee_size = guardian_ids().len();

        most_set_committee(
            &mut client,
            &alice(),
            most_address,
            &guardian_ids()[1..],
            DEFAULT_THRESHOLD - 1,
        )
        .await
        .expect("can set committee");

        let member_id = guardian_ids()[0];

        let guardian_balance_before = client
            .balance(member_id)
            .await
            .expect("guardian balance before");

        most_request_payout(
            &mut client,
            &alice(),
            most_address,
            previous_committee_id,
            member_id,
        )
        .await
        .expect("request payout");

        let total_rewards =
            most_committee_rewards(&mut client, most_address, previous_committee_id)
                .await
                .expect("committee rewards");

        let guardian_balance_after = client
            .balance(member_id)
            .await
            .expect("guardian balance after");

        assert_eq!(
            guardian_balance_after,
            guardian_balance_before + (total_rewards / previous_committee_size as u128)
        );
    }

    #[ink_e2e::test]
    fn use_gas_oracle(mut client: ink_e2e::Client<C, E>) {
        let (most_address, _token_address) = setup_default_most_and_token(&mut client, true).await;

        let base_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        assert_eq!(base_fee, DEFAULT_FEE);

        // Oracle returning price withing the range
        let oracle_address = instantiate_oracle(
            &mut client,
            &alice(),
            2 * DEFAULT_FEE / DEFAULT_RELAY_GAS_USAGE,
        )
        .await;
        most_set_gas_oracle(&mut client, &alice(), most_address, oracle_address)
            .await
            .expect("can set gas oracle");

        let oracle_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        assert_eq!(oracle_fee, 2 * DEFAULT_FEE * 120 / 100);

        // Oracle returning price larger than the maximum allowed price
        let oracle_address =
            instantiate_oracle(&mut client, &alice(), 2 * MAX_FEE / DEFAULT_RELAY_GAS_USAGE).await;

        most_set_gas_oracle(&mut client, &alice(), most_address, oracle_address)
            .await
            .expect("can set gas oracle");

        let oracle_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        assert_eq!(oracle_fee, MAX_FEE);

        // Oracle returning price smaller than the minimum allowed price
        let oracle_address = instantiate_oracle(
            &mut client,
            &alice(),
            MIN_FEE / (2 * DEFAULT_RELAY_GAS_USAGE),
        )
        .await;

        most_set_gas_oracle(&mut client, &alice(), most_address, oracle_address)
            .await
            .expect("can set gas oracle");

        let oracle_fee = most_base_fee(&mut client, most_address)
            .await
            .expect("should return base fee");

        assert_eq!(oracle_fee, MIN_FEE);
    }

    #[ink_e2e::test]
    fn committee_change(mut client: ink_e2e::Client<C, E>) {
        let (most_address, token_address) = setup_default_most_and_token(&mut client, true).await;

        most_set_committee(
            &mut client,
            &alice(),
            most_address,
            &guardian_ids()[1..],
            DEFAULT_THRESHOLD - 1,
        )
        .await
        .expect("can set committee");

        let old_committee_id = 1;

        most_set_committee(
            &mut client,
            &alice(),
            most_address,
            &guardian_ids()[..guardian_ids().len() - 1],
            DEFAULT_THRESHOLD - 1,
        )
        .await
        .expect("can set committee");

        let new_committee_id = 2;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 0;

        let old_request_hash = hash_request_data(
            old_committee_id,
            token_address,
            amount,
            receiver_address,
            request_nonce,
        );

        let new_request_hash = hash_request_data(
            new_committee_id,
            token_address,
            amount,
            receiver_address,
            request_nonce + 1,
        );

        let old_request_new_guardian = most_receive_request(
            &mut client,
            &guardian_keys()[0],
            most_address,
            old_request_hash,
            old_committee_id,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        let new_request_old_guardian = most_receive_request(
            &mut client,
            &guardian_keys()[guardian_ids().len() - 1],
            most_address,
            new_request_hash,
            new_committee_id,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce + 1,
        )
        .await;

        let new_request_new_guardian = most_receive_request(
            &mut client,
            &guardian_keys()[0],
            most_address,
            new_request_hash,
            new_committee_id,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce + 1,
        )
        .await;

        let old_request_old_guardian = most_receive_request(
            &mut client,
            &guardian_keys()[guardian_ids().len() - 1],
            most_address,
            old_request_hash,
            old_committee_id,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        assert_eq!(
            old_request_new_guardian.expect_err("Receive request should fail for non-guardians"),
            MostError::NotInCommittee
        );

        assert_eq!(
            new_request_old_guardian.expect_err("Receive request should fail for non-guardians"),
            MostError::NotInCommittee
        );

        assert!(new_request_new_guardian.is_ok());

        assert!(old_request_old_guardian.is_ok());
    }

    fn guardian_ids() -> Vec<AccountId> {
        vec![
            account_id(AccountKeyring::Bob),
            account_id(AccountKeyring::Charlie),
            account_id(AccountKeyring::Dave),
            account_id(AccountKeyring::Eve),
            account_id(AccountKeyring::Ferdie),
        ]
    }

    fn guardian_keys() -> Vec<Keypair> {
        vec![bob(), charlie(), dave(), eve(), ferdie()]
    }

    fn hash_request_data(
        commitee_id: CommitteeId,
        token_address: AccountId,
        amount: u128,
        receiver_address: AccountId,
        request_nonce: u128,
    ) -> Keccak256HashOutput {
        let request_data = [
            &commitee_id.to_le_bytes(),
            AsRef::<[u8]>::as_ref(&token_address),
            &amount.to_le_bytes(),
            AsRef::<[u8]>::as_ref(&receiver_address),
            &request_nonce.to_le_bytes(),
        ]
        .concat();
        keccak256(&request_data)
    }

    #[derive(Debug)]
    struct CallResultValue<V> {
        value: V,
        events: Vec<EventWithTopics<ContractEmitted>>,
    }

    type CallResult<V, E> = Result<CallResultValue<V>, E>;
    type E2EClient = ink_e2e::Client<PolkadotConfig, DefaultEnvironment>;

    #[allow(clippy::too_many_arguments)]
    async fn instantiate_most(
        client: &mut E2EClient,
        caller: &Keypair,
        guardians: Vec<AccountId>,
        threshold: u128,
        pocket_money: u128,
        relay_gas_usage: u128,
        min_fee: u128,
        max_fee: u128,
        default_fee: u128,
    ) -> AccountId {
        let most_constructor = MostRef::new(
            guardians,
            threshold,
            pocket_money,
            relay_gas_usage,
            min_fee,
            max_fee,
            default_fee,
            None,
        );
        client
            .instantiate("most", caller, most_constructor, 0, None)
            .await
            .expect("Most instantiation failed")
            .account_id
    }

    async fn instantiate_token(
        client: &mut E2EClient,
        caller: &Keypair,
        total_supply: u128,
        decimals: u8,
        admin: AccountId,
    ) -> AccountId {
        let token_constructor = TokenRef::new(total_supply, None, None, decimals, admin);
        client
            .instantiate("token", caller, token_constructor, 0, None)
            .await
            .expect("Token instantiation failed")
            .account_id
    }

    async fn instantiate_oracle(
        client: &mut E2EClient,
        caller: &Keypair,
        price: u128,
    ) -> AccountId {
        let oracle_constructor = OracleRef::new(account_id(AccountKeyring::Alice), price);
        client
            .instantiate("oracle", caller, oracle_constructor, 0, None)
            .await
            .expect("Oracle instantiation failed")
            .account_id
    }

    async fn setup_default_most_and_token(
        client: &mut E2EClient,
        add_pair: bool,
    ) -> (AccountId, AccountId) {
        let most_address = instantiate_most(
            client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            DEFAULT_POCKET_MONEY,
            DEFAULT_RELAY_GAS_USAGE,
            MIN_FEE,
            MAX_FEE,
            DEFAULT_FEE,
        )
        .await;

        let token_address = instantiate_token(
            client,
            &alice(),
            TOKEN_INITIAL_SUPPLY,
            DECIMALS,
            most_address,
        )
        .await;

        if add_pair {
            most_add_pair(client, &alice(), most_address, token_address, REMOTE_TOKEN)
                .await
                .expect("Add pair should succeed");
        }

        (most_address, token_address)
    }

    async fn most_add_pair(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        token: AccountId,
        remote_token: [u8; 32],
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, (), _, _, _>(
            client,
            caller,
            most,
            |most| most.add_pair(*token.as_ref(), remote_token),
            None,
        )
        .await
    }

    async fn most_set_gas_oracle(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        oracle: AccountId,
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, (), _, _, _>(
            client,
            caller,
            most,
            |most| most.set_gas_price_oracle(oracle),
            None,
        )
        .await
    }

    async fn most_set_committee(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        members: &[AccountId],
        threshold: u128,
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, _, _, _, _>(
            client,
            caller,
            most,
            |most| most.set_committee(members.to_vec(), threshold),
            None,
        )
        .await
    }

    async fn most_send_request(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        token: AccountId,
        amount: u128,
        receiver_address: [u8; 32],
        base_fee: u128,
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, (), _, _, _>(
            client,
            caller,
            most,
            |most| most.send_request(*token.as_ref(), amount, receiver_address),
            Some(base_fee),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn most_receive_request(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        request_hash: Keccak256HashOutput,
        committee_id: CommitteeId,
        token: [u8; 32],
        amount: u128,
        receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, (), _, _, _>(
            client,
            caller,
            most,
            |most| {
                most.receive_request(
                    request_hash,
                    committee_id,
                    token,
                    amount,
                    receiver_address,
                    request_nonce,
                )
            },
            None,
        )
        .await
    }

    async fn most_request_payout(
        client: &mut E2EClient,
        caller: &Keypair,
        most: AccountId,
        committee_id: u128,
        member_id: AccountId,
    ) -> CallResult<(), MostError> {
        call_message::<MostRef, _, _, _, _>(
            client,
            caller,
            most,
            |most| most.payout_rewards(committee_id, member_id),
            None,
        )
        .await
    }

    async fn most_base_fee(client: &mut E2EClient, most: AccountId) -> Result<u128, MostError> {
        call_message::<MostRef, u128, _, _, _>(
            client,
            &alice(),
            most,
            |most| most.get_base_fee(),
            None,
        )
        .await
        .map(|call_res| call_res.value)
    }

    async fn psp22_balance_of(
        client: &mut E2EClient,
        token: AccountId,
        owner: AccountId,
    ) -> Result<u128, PSP22Error> {
        let balance_of_call =
            build_message::<TokenRef>(token).call(|token| token.balance_of(owner));

        Ok(client
            .call_dry_run(&alice(), &balance_of_call, 0, None)
            .await
            .return_value())
    }

    async fn psp22_total_supply(
        client: &mut E2EClient,
        token: AccountId,
    ) -> Result<u128, PSP22Error> {
        let call = build_message::<TokenRef>(token).call(|token| token.total_supply());

        Ok(client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value())
    }

    async fn psp22_approve(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        spender: AccountId,
        amount: u128,
    ) -> CallResult<(), PSP22Error> {
        call_message::<TokenRef, _, _, _, _>(
            client,
            caller,
            token,
            |token| token.approve(spender, amount),
            None,
        )
        .await
    }

    async fn most_committee_rewards(
        client: &mut E2EClient,
        most_address: AccountId,
        committee_id: u128,
    ) -> Result<u128, MostError> {
        let call = build_message::<MostRef>(most_address)
            .call(|most| most.get_collected_committee_rewards(committee_id));

        Ok(client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value())
    }

    async fn most_committee_id(
        client: &mut E2EClient,
        most_address: AccountId,
    ) -> Result<u128, MostError> {
        let call =
            build_message::<MostRef>(most_address).call(|most| most.get_current_committee_id());

        client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value()
    }

    async fn call_message<Ref, RetType, ErrType, Args, F>(
        client: &mut E2EClient,
        caller: &Keypair,
        contract_id: AccountId,
        call_builder_fn: F,
        value: Option<u128>,
    ) -> CallResult<RetType, ErrType>
    where
        Ref: TraitCallBuilder + FromAccountId<DefaultEnvironment>,
        F: Clone
            + FnMut(
                &mut <Ref as TraitCallBuilder>::Builder,
            ) -> CallBuilder<
                DefaultEnvironment,
                Set<Call<DefaultEnvironment>>,
                Set<ExecutionInput<Args>>,
                Set<ReturnType<Result<RetType, ErrType>>>,
            >,
        Args: Encode,
        ErrType: Decode + Debug,
        RetType: Decode,
    {
        let message = build_message::<Ref>(contract_id).call(call_builder_fn);

        // Dry run to get the return value: when a contract is called and reverted, then we
        // get a large error message that is not very useful. We want to get the actual contract
        // error and this can be done by dry running the call.
        client
            .call_dry_run(caller, &message, value.unwrap_or_default(), None)
            .await
            .return_value()?;

        // Now we shouldn't get any errors originating from the contract.
        // However, we can still get errors from the substrate runtime or the client.
        let call_result = client
            .call(caller, message, value.unwrap_or_default(), None)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Call did not revert, but failed anyway. ink_e2e error: {:?}",
                    err
                )
            });
        Ok(CallResultValue {
            value: call_result
                .dry_run
                .return_value()
                .expect("return value should be present"),
            events: get_contract_emitted_events(call_result.events)
                .expect("event decoding should not fail"),
        })
    }
}
