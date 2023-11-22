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
        account_id, alice, bob, build_message, charlie, dave, eve, ferdie, AccountKeyring, Keypair,
        PolkadotConfig,
    };
    use membrane::{membrane::CrosschainTransferRequest, MembraneError, MembraneRef};
    use psp22::{PSP22Error, PSP22};
    use scale::{Decode, Encode};
    use shared::{keccak256, Keccak256HashOutput};
    use wrapped_token::TokenRef;

    use crate::events::{
        filter_decode_events_as, get_contract_emitted_events, ContractEmitted, EventWithTopics,
    };

    const TOKEN_INITIAL_SUPPLY: u128 = 10000;
    const DEFAULT_THRESHOLD: u128 = 3;
    const DECIMALS: u8 = 8;
    const REMOTE_TOKEN: [u8; 32] = [0x1; 32];
    const REMOTE_RECEIVER: [u8; 32] = [0x2; 32];

    #[ink_e2e::test]
    fn simple_deploy_works(mut client: ink_e2e::Client<C, E>) {
        let _membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;
    }

    #[ink_e2e::test]
    fn owner_can_add_a_new_pair(mut client: ink_e2e::Client<C, E>) {
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

        membrane_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            REMOTE_TOKEN,
        )
        .await
        .expect("Adding a pair should succeed");

        let amount_to_send = 1000;
        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
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
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
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
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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

        let send_request_res = membrane_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            amount_to_send,
            REMOTE_RECEIVER,
        )
        .await;

        match send_request_res {
            Ok(events) => {
                // 2 events for transfer_from and 1 `CrosschainTransferRequest`
                assert_eq!(events.len(), 3);

                let request_events =
                    filter_decode_events_as::<CrosschainTransferRequest>(events);

                // `CrosschainTransferRequest` event
                assert_eq!(request_events.len(), 1);
                assert_eq!(request_events[0], CrosschainTransferRequest {
                    dest_token_address: REMOTE_TOKEN,
                    amount: amount_to_send,
                    dest_receiver_address: REMOTE_RECEIVER,
                    request_nonce: 0,
                });
            }
            Err(e) => panic!("Request should succeed: {:?}", e),
        }
    }

    #[ink_e2e::test]
    fn receive_request_can_only_be_called_by_guardians(mut client: ink_e2e::Client<C, E>) {
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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
            MembraneError::NotGuardian(account_id(AccountKeyring::Alice))
        );
    }

    #[ink_e2e::test]
    fn receive_request_non_matching_hash(mut client: ink_e2e::Client<C, E>) {
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;

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
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), DEFAULT_THRESHOLD).await;
        psp22_transfer(&mut client, &alice(), token_address, 100, membrane_address)
            .await
            .expect("Transfer should succeed");

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        for signer in &guardian_keys()[0..(DEFAULT_THRESHOLD as usize)] {
            membrane_receive_request(
                &mut client,
                &signer,
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

        assert_eq!(balance, amount);
    }

    #[ink_e2e::test]
    fn receive_request_not_enough_signatures(mut client: ink_e2e::Client<C, E>) {
        let guardians_threshold = 5;
        let token_address =
            instantiate_token(&mut client, &alice(), TOKEN_INITIAL_SUPPLY, DECIMALS).await;
        let membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), guardians_threshold).await;
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
                &signer,
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
            &(amount as u128).to_le_bytes(),
            AsRef::<[u8]>::as_ref(&receiver_address),
            &(request_nonce as u128).to_le_bytes(),
        ]
        .concat();
        keccak256(&request_data)
    }

    type CallResult<E> = Result<Vec<EventWithTopics<ContractEmitted>>, E>;
    type E2EClient = ink_e2e::Client<PolkadotConfig, DefaultEnvironment>;

    async fn instantiate_membrane(
        client: &mut E2EClient,
        caller: &Keypair,
        guardians: Vec<AccountId>,
        threshold: u128,
    ) -> AccountId {
        let membrane_constructor = MembraneRef::new(guardians, threshold);
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
    ) -> CallResult<MembraneError> {
        call_message::<MembraneRef, _, _, _>(client, caller, membrane, |membrane| {
            membrane.add_pair(*token.as_ref(), remote_token)
        })
        .await
    }

    async fn membrane_send_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        token: AccountId,
        amount: u128,
        remote_address: [u8; 32],
    ) -> CallResult<MembraneError> {
        call_message::<MembraneRef, _, _, _>(client, caller, membrane, |membrane| {
            membrane.send_request(*token.as_ref(), amount, remote_address)
        })
        .await
    }

    async fn membrane_receive_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        request_hash: Keccak256HashOutput,
        token: [u8; 32],
        amount: u128,
        receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> CallResult<MembraneError> {
        call_message::<MembraneRef, _, _, _>(client, caller, membrane, |membrane| {
            membrane.receive_request(request_hash, token, amount, receiver_address, request_nonce)
        })
        .await
    }

    async fn psp22_approve(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        spender: AccountId,
    ) -> CallResult<PSP22Error> {
        call_message::<TokenRef, _, _, _>(client, caller, token, |token| {
            token.approve(spender, amount)
        })
        .await
    }

    async fn psp22_transfer(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        recipient: AccountId,
    ) -> CallResult<PSP22Error> {
        call_message::<TokenRef, _, _, _>(client, caller, token, |token| {
            token.transfer(recipient, amount, vec![])
        })
        .await
    }

    async fn call_message<Ref, ErrType, Args, F>(
        client: &mut E2EClient,
        caller: &Keypair,
        contract_id: AccountId,
        call_builder_fn: F,
    ) -> CallResult<ErrType>
    where
        Ref: TraitCallBuilder + FromAccountId<DefaultEnvironment>,
        F: FnMut(
            &mut <Ref as TraitCallBuilder>::Builder,
        ) -> CallBuilder<
            DefaultEnvironment,
            Set<Call<DefaultEnvironment>>,
            Set<ExecutionInput<Args>>,
            Set<ReturnType<Result<(), ErrType>>>,
        >,
        Args: Encode,
        ErrType: Decode,
    {
        let message = build_message::<Ref>(contract_id).call(call_builder_fn);

        // Dry run to get the return value: when a contract is called and reverted, then we
        // get a large error message that is not very useful. We want to get the actual contract
        // error and this can be done by dry running the call.
        client
            .call_dry_run(caller, &message, 0, None)
            .await
            .return_value()?;

        // Now we shouldn't get any errors originating from the contract.
        // However, we can still get errors from the substrate runtime or the client.
        let call_result = client
            .call(caller, message, 0, None)
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Call did not revert, but failed anyway. ink_e2e error: {:?}",
                    err
                )
            });
        Ok(
            get_contract_emitted_events(call_result.events)
                .expect("event decoding should not fail"),
        )
    }
}
