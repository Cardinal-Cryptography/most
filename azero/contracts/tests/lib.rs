#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(all(test, feature = "e2e-tests"))]
mod e2e {
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
    use membrane::{MembraneError, MembraneRef};
    use psp22::{PSP22Error, PSP22};
    use scale::{Decode, Encode};
    use shared::{keccak256, Keccak256HashOutput};
    use wrapped_token::TokenRef;

    const TOKEN_INITIAL_SUPPLY: u128 = 10000;
    const DEFAULT_THRESHOLD: u128 = 3;
    const DECIMALS: u8 = 8;
    const REMOTE_TOKEN: [u8; 32] = [0x1; 32];
    const REMOTE_RECEIVER: [u8; 32] = [0x2; 32];

    const USDT_TOKEN_ID: [u8; 32] = [0x2; 32];

    #[ink_e2e::test]
    fn simple_deploy_works(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let _membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;
    }

    #[ink_e2e::test]
    fn owner_can_add_a_new_pair(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let add_pair_res = membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await;

        assert!(add_pair_res.is_ok());
    }

    #[ink_e2e::test]
    fn non_owner_cannot_add_a_new_pair(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let add_pair_res = membrane_add_pair(
            &mut client,
            &bob(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await;

        assert_eq!(
            add_pair_res
                .err()
                .expect("Bob should not be able to add a pair as he is not the owner"),
            MembraneError::NotOwner(account_id(AccountKeyring::Bob))
        );
    }

    #[ink_e2e::test]
    fn send_request_fails_without_allowance(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await
        .expect("Adding a pair should succeed");

        let base_fee = membrane_base_fee(&mut client, &alice(), membrane_address)
            .await
            .expect("should return base fee");

        let amount_to_send = 1000;
        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail without allowance"),
            MembraneError::PSP22(PSP22Error::InsufficientAllowance)
        );
    }

    #[ink_e2e::test]
    fn send_request_fails_on_non_whitelisted_token(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let amount_to_send = 1000;
        psp22_approve(
            &mut client,
            &alice(),
            token_address,
            amount_to_send,
            membrane_address,
        )
        .await
        .expect("Approve should succeed");

        let base_fee = membrane_base_fee(&mut client, &alice(), membrane_address)
            .await
            .expect("base fee");

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail for a non-whitelisted token"),
            MembraneError::UnsupportedPair
        );
    }

    #[ink_e2e::test]
    fn correct_request(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 300;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let amount_to_send = 1000;
        psp22_approve(
            &mut client,
            &alice(),
            token_address,
            amount_to_send,
            membrane_address,
        )
        .await
        .expect("Approve should succeed");

        membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await
        .expect("Adding a pair should succeed");

        let base_fee = membrane_base_fee(&mut client, &alice(), membrane_address)
            .await
            .expect("should return base fee");

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        assert!(send_request_res.is_ok());
    }

    #[ink_e2e::test]
    fn receive_request_can_only_be_called_by_guardians(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        let alice_receive_request_res = membrane_receive_request(
            &mut client,
            &alice(),
            membrane_address,
            request_hash,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        assert_eq!(
            alice_receive_request_res
                .err()
                .expect("Receive request should fail for non-guardians"),
            MembraneError::NotInCommittee
        );
    }

    #[ink_e2e::test]
    fn receive_request_non_matching_hash(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let incorrect_hash = [0x3; 32];
        let receive_request_res = membrane_receive_request(
            &mut client,
            &bob(),
            membrane_address,
            incorrect_hash,
            *token_address.as_ref(),
            amount,
            *receiver_address.as_ref(),
            request_nonce,
        )
        .await;

        assert_eq!(
            receive_request_res
                .err()
                .expect("Receive request should fail for non-matching hash"),
            MembraneError::HashDoesNotMatchData
        );
    }

    #[ink_e2e::test]
    fn receive_request_executes_request_after_enough_transactions(
        mut client: ink_e2e::Client<C, E>,
    ) {
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let amount = 20; // 841189100000000
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            membrane_receive_request(
                &mut client,
                signer,
                membrane_address,
                request_hash,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await
            .expect("Receive request should succeed");
        }

        let balance = psp22_balance_of(&mut client, token_address, receiver_address)
            .await
            .expect("balance before");

        println!("balance: {balance}");

        // TODO : why you fail?
        assert_eq!(
            balance,
            ((amount * (10000 - commission_per_dix_mille)) / 10000)
        );
    }

    #[ink_e2e::test]
    fn receive_request_not_enough_signatures(mut client: ink_e2e::Client<C, E>) {
        let guardians_threshold = 5;
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            guardians_threshold,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;
        psp22_transfer(&mut client, &alice(), token_address, 100, membrane_address)
            .await
            .expect("Transfer should succeed");

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        for signer in &guardian_keys()[0..(guardians_threshold as usize) - 1] {
            membrane_receive_request(
                &mut client,
                signer,
                membrane_address,
                request_hash,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await
            .expect("Receive request should succeed");
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
    fn amount_below_minimum(mut client: ink_e2e::Client<C, E>) {
        let guardians_threshold = 5;
        let commission_per_dix_mille = 300;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            guardians_threshold,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await
        .expect("Adding a pair should succeed");

        let base_fee = membrane_base_fee(&mut client, &alice(), membrane_address)
            .await
            .expect("should return base fee");

        // amount is set by query_fee result
        let amount_to_send = membrane_query_price(
            &mut client,
            membrane_address,
            minimum_transfer_amount_usd - 1,
            USDT_TOKEN_ID, // of
            REMOTE_TOKEN,  // in
        )
        .await
        .expect("price query result");

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
            base_fee,
        )
        .await;

        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail without allowance"),
            MembraneError::AmountBelowMinimum
        );
    }

    #[ink_e2e::test]
    fn base_fee_too_low(mut client: ink_e2e::Client<C, E>) {
        let guardians_threshold = 5;
        let commission_per_dix_mille = 30;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            guardians_threshold,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await
        .expect("Adding a pair should succeed");

        let base_fee = membrane_base_fee(&mut client, &alice(), membrane_address)
            .await
            .expect("should return base fee");

        let amount = 841189100000000;

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount,
            REMOTE_RECEIVER,
            base_fee - 1,
        )
        .await;

        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail without allowance"),
            MembraneError::BaseFeeTooLow
        );
    }

    #[ink_e2e::test]
    fn pocket_money(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 300;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        // seed contract with some funds for pocket money transfers
        let call_data = vec![
            Value::unnamed_variant("Id", [Value::from_bytes(membrane_address)]),
            Value::u128(10 * pocket_money),
        ];

        client
            .runtime_call(&alice(), "Balances", "transfer", call_data)
            .await
            .expect("runtime call failed");

        let amount = 841189100000000;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        let azero_balance_before = client
            .balance(receiver_address)
            .await
            .expect("native balance before");

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            membrane_receive_request(
                &mut client,
                signer,
                membrane_address,
                request_hash,
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

        assert_eq!(azero_balance_after, azero_balance_before + pocket_money);
    }

    #[ink_e2e::test]
    fn committee_rewards(mut client: ink_e2e::Client<C, E>) {
        let commission_per_dix_mille = 300;
        let pocket_money = 1000000000000;
        let minimum_transfer_amount_usd = 50;
        let relay_gas_usage = 50000;

        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;

        let membrane_address = instantiate_membrane(
            &mut client,
            &alice(),
            guardian_ids(),
            DEFAULT_THRESHOLD,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        )
        .await;

        let commission = membrane_commission_per_dix_mille(&mut client, membrane_address).await;

        assert_eq!(commission, commission_per_dix_mille);

        let amount = 841189100000000;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        let token_balance_before = psp22_balance_of(&mut client, token_address, receiver_address)
            .await
            .expect("balance before");

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            membrane_receive_request(
                &mut client,
                signer,
                membrane_address,
                request_hash,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await
            .expect("Receive request should succeed");
        }

        let token_balance_after = psp22_balance_of(&mut client, token_address, receiver_address)
            .await
            .expect("balance after");

        assert_eq!(
            token_balance_after,
            token_balance_before + ((amount * (10000 - commission)) / 10000)
        );

        let committee_id = membrane_committee_id(&mut client, membrane_address)
            .await
            .expect("committe id");

        let total_rewards = membrane_committee_rewards(
            &mut client,
            membrane_address,
            committee_id,
            *token_address.as_ref(),
        )
        .await
        .expect("committee rewards");

        assert_eq!(total_rewards, (amount * commission) / 10000);

        let committee_size = guardian_ids().len();
        for i in 0..committee_size {
            let signer = &guardian_keys()[i];
            let member_id = guardian_ids()[i];

            let signer_balance_before = psp22_balance_of(&mut client, token_address, member_id)
                .await
                .expect("signer balance before");

            membrane_request_payout(
                &mut client,
                signer,
                membrane_address,
                committee_id,
                member_id,
                *token_address.as_ref(),
            )
            .await
            .expect("request payout");

            let signer_balance_after = psp22_balance_of(&mut client, token_address, member_id)
                .await
                .expect("signer balance after");

            assert_eq!(
                signer_balance_after,
                signer_balance_before + (total_rewards / committee_size as u128)
            );
        }

        // no double spend is possible
        let bob_balance_before =
            psp22_balance_of(&mut client, token_address, account_id(AccountKeyring::Bob))
                .await
                .expect("signer balance before");

        membrane_request_payout(
            &mut client,
            &alice(),
            membrane_address,
            committee_id,
            account_id(AccountKeyring::Bob),
            *token_address.as_ref(),
        )
        .await
        .expect("request payout twice");

        let bob_balance_after =
            psp22_balance_of(&mut client, token_address, account_id(AccountKeyring::Bob))
                .await
                .expect("signer balance before");

        assert_eq!(bob_balance_after, bob_balance_before);
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
        token_address: AccountId,
        amount: u128,
        receiver_address: AccountId,
        request_nonce: u128,
    ) -> Keccak256HashOutput {
        let request_data = [
            AsRef::<[u8]>::as_ref(&token_address),
            &amount.to_le_bytes(),
            AsRef::<[u8]>::as_ref(&receiver_address),
            &request_nonce.to_le_bytes(),
        ]
        .concat();
        keccak256(&request_data)
    }

    // type DryRunResult<V, E> = ink_e2e::CallDryRunResult<DefaultEnvironment, Result<V, E>>;
    type CallResult<V, E> =
        Result<ink_e2e::CallResult<PolkadotConfig, DefaultEnvironment, Result<V, E>>, E>;
    type E2EClient = ink_e2e::Client<PolkadotConfig, DefaultEnvironment>;

    #[allow(clippy::too_many_arguments)]
    async fn instantiate_membrane(
        client: &mut E2EClient,
        caller: &Keypair,
        guardians: Vec<AccountId>,
        threshold: u128,
        commission_per_dix_mille: u128,
        pocket_money: u128,
        minimum_transfer_amount_usd: u128,
        relay_gas_usage: u128,
    ) -> AccountId {
        let membrane_constructor = MembraneRef::new(
            guardians,
            threshold,
            commission_per_dix_mille,
            pocket_money,
            minimum_transfer_amount_usd,
            relay_gas_usage,
        );
        client
            .instantiate("membrane", caller, membrane_constructor, 0, None)
            .await
            .expect("Membrane instantiation failed")
            .account_id
    }

    async fn instantiate_token(
        client: &mut E2EClient,
        caller: &Keypair,
        total_supply: u128,
        decimals: u8,
    ) -> AccountId {
        let token_constructor = TokenRef::new(total_supply, None, None, decimals);
        client
            .instantiate("token", caller, token_constructor, 0, None)
            .await
            .expect("Token instantiation failed")
            .account_id
    }

    async fn membrane_add_pair(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        token: AccountId,
        remote_token: [u8; 32],
    ) -> CallResult<(), MembraneError> {
        call_message::<MembraneRef, (), _, _, _>(
            client,
            caller,
            membrane,
            |membrane| membrane.add_pair(*token.as_ref(), remote_token),
            None,
        )
        .await
    }

    async fn membrane_send_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        token: AccountId,
        amount: u128,
        remote_address: [u8; 32],
        base_fee: u128,
    ) -> CallResult<(), MembraneError> {
        call_message::<MembraneRef, (), _, _, _>(
            client,
            caller,
            membrane,
            |membrane| membrane.send_request(*token.as_ref(), amount, remote_address),
            Some(base_fee),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn membrane_receive_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        request_hash: Keccak256HashOutput,
        token: [u8; 32],
        amount: u128,
        receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> CallResult<(), MembraneError> {
        call_message::<MembraneRef, (), _, _, _>(
            client,
            caller,
            membrane,
            |membrane| {
                membrane.receive_request(
                    request_hash,
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

    async fn psp22_approve(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        spender: AccountId,
    ) -> CallResult<(), PSP22Error> {
        call_message::<TokenRef, (), _, _, _>(
            client,
            caller,
            token,
            |token| token.approve(spender, amount),
            None,
        )
        .await
    }

    async fn psp22_transfer(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        recipient: AccountId,
    ) -> CallResult<(), PSP22Error> {
        call_message::<TokenRef, (), _, _, _>(
            client,
            caller,
            token,
            |token| token.transfer(recipient, amount, vec![]),
            None,
        )
        .await
    }

    async fn membrane_request_payout(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        committee_id: u128,
        member_id: AccountId,
        token_id: [u8; 32],
    ) -> CallResult<(), MembraneError> {
        call_message::<MembraneRef, _, _, _, _>(
            client,
            caller,
            membrane,
            |membrane| membrane.payout_rewards(committee_id, member_id, token_id),
            None,
        )
        .await
    }

    async fn membrane_base_fee(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
    ) -> Result<u128, MembraneError> {
        call_message::<MembraneRef, u128, _, _, _>(
            client,
            caller,
            membrane,
            |membrane| membrane.base_fee(),
            None,
        )
        .await
        .expect("oooops")
        .dry_run
        .return_value()
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

    async fn membrane_committee_rewards(
        client: &mut E2EClient,
        membrane_address: AccountId,
        committee_id: u128,
        token_id: [u8; 32],
    ) -> Result<u128, MembraneError> {
        let call = build_message::<MembraneRef>(membrane_address)
            .call(|membrane| membrane.get_committee_rewards(committee_id, token_id));

        Ok(client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value())
    }

    async fn membrane_committee_id(
        client: &mut E2EClient,
        membrane_address: AccountId,
    ) -> Result<u128, MembraneError> {
        let call = build_message::<MembraneRef>(membrane_address)
            .call(|membrane| membrane.current_committee_id());

        Ok(client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value())
    }

    async fn membrane_commission_per_dix_mille(
        client: &mut E2EClient,
        membrane_address: AccountId,
    ) -> u128 {
        let call = build_message::<MembraneRef>(membrane_address)
            .call(|membrane| membrane.get_commission_per_dix_mille());

        client
            .call_dry_run(&alice(), &call, 0, None)
            .await
            .return_value()
    }

    async fn membrane_query_price(
        client: &mut E2EClient,
        membrane_address: AccountId,
        amount_of: u128,
        of_token: [u8; 32],
        in_token: [u8; 32],
    ) -> Result<u128, MembraneError> {
        let call = build_message::<MembraneRef>(membrane_address)
            .call(|membrane| membrane.query_price(amount_of, of_token, in_token));

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
        ErrType: Decode,
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
        Ok(client
            .call(caller, message, value.unwrap_or_default(), None)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Call did not revert, but failed anyway. ink_e2e error: {:?}",
                    err
                )
            }))
    }
}
