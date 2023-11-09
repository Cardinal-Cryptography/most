#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(all(test, feature = "e2e-tests"))]
mod e2e {
    use std::error::Error;

    use ink::{env::DefaultEnvironment, primitives::AccountId};
    use ink_e2e::{
        account_id, alice, bob, build_message, charlie, dave, eve, ferdie, AccountKeyring, Keypair,
        PolkadotConfig,
    };
    use membrane::{MembraneError, MembraneRef};
    use psp22::{PSP22Error, PSP22};
    use shared::{keccak256, Keccak256HashOutput};
    use wrapped_token::TokenRef;

    #[ink_e2e::test]
    fn simple_deploy_works(mut client: ink_e2e::Client<C, E>) -> Result<(), Box<dyn Error>> {
        let _membrane_address =
            instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;
        Ok(())
    }

    #[ink_e2e::test]
    fn adding_pair_works(mut client: ink_e2e::Client<C, E>) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let bob_res = call_add_pair(
            &mut client,
            &bob(),
            membrane_address,
            token_address,
            [0x0; 32],
        )
        .await;

        let alice_res = call_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            [0x0; 32],
        )
        .await;

        assert_eq!(
            bob_res.err().expect("Bob should not be able to add a pair"),
            MembraneError::NotOwner(account_id(AccountKeyring::Bob))
        );
        assert!(alice_res.is_ok());

        Ok(())
    }

    #[ink_e2e::test]
    fn send_request_fails_without_allowance(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let add_pair_res = call_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            [0x0; 32],
        )
        .await;

        let send_request_res = call_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            1000,
            [0x1; 32],
        )
        .await;

        assert!(add_pair_res.is_ok());
        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail without allowance"),
            MembraneError::PSP22(PSP22Error::InsufficientAllowance)
        );

        Ok(())
    }

    #[ink_e2e::test]
    fn send_request_fails_on_non_whitelisted_token(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let approve_res =
            call_approve(&mut client, &alice(), token_address, 1000, membrane_address).await;

        let send_request_res = call_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            1000,
            [0x1; 32],
        )
        .await;

        assert!(approve_res.is_ok());
        assert_eq!(
            send_request_res
                .err()
                .expect("Request should fail for a non-whitelisted token"),
            MembraneError::UnsupportedPair
        );

        Ok(())
    }

    #[ink_e2e::test]
    fn correct_request(mut client: ink_e2e::Client<C, E>) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let approve_res =
            call_approve(&mut client, &alice(), token_address, 1000, membrane_address).await;

        let add_pair_res = call_add_pair(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            [0x0; 32],
        )
        .await;

        let send_request_res = call_send_request(
            &mut client,
            &alice(),
            membrane_address,
            token_address,
            1000,
            [0x1; 32],
        )
        .await;

        assert!(approve_res.is_ok());
        assert!(add_pair_res.is_ok());
        assert!(send_request_res.is_ok());

        Ok(())
    }

    #[ink_e2e::test]
    fn receive_request_can_only_be_called_by_guardians(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        let alice_receive_request_res = call_receive_request(
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

        Ok(())
    }

    #[ink_e2e::test]
    fn receive_request_non_matching_hash(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let receive_request_res = call_receive_request(
            &mut client,
            &bob(),
            membrane_address,
            [0x1; 32],
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

        Ok(())
    }

    #[ink_e2e::test]
    fn receive_request_executes_request_after_enough_transactions(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 3).await;
        call_transfer(&mut client, &alice(), token_address, 100, membrane_address)
            .await
            .expect("Transfer should succeed");

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        for signer in &guardian_keys()[0..3] {
            let receive_request_res = call_receive_request(
                &mut client,
                &signer,
                membrane_address,
                request_hash,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await;

            assert!(receive_request_res.is_ok());
        }

        let balance_of_call = build_message::<TokenRef>(token_address)
            .call(|token| token.balance_of(receiver_address));
        let balance = client
            .call_dry_run(&alice(), &balance_of_call, 0, None)
            .await
            .return_value();

        assert_eq!(balance, amount);

        Ok(())
    }

    #[ink_e2e::test]
    fn receive_request_not_enough_signatures(
        mut client: ink_e2e::Client<C, E>,
    ) -> Result<(), Box<dyn Error>> {
        let token_address = instantiate_token(&mut client, &alice(), 10000, 8).await;
        let membrane_address = instantiate_membrane(&mut client, &alice(), guardian_ids(), 5).await;
        call_transfer(&mut client, &alice(), token_address, 100, membrane_address)
            .await
            .expect("Transfer should succeed");

        let amount = 20;
        let receiver_address = account_id(AccountKeyring::One);
        let request_nonce = 1;

        let request_hash =
            hash_request_data(token_address, amount, receiver_address, request_nonce);

        for signer in &guardian_keys()[0..4] {
            let receive_request_res = call_receive_request(
                &mut client,
                &signer,
                membrane_address,
                request_hash,
                *token_address.as_ref(),
                amount,
                *receiver_address.as_ref(),
                request_nonce,
            )
            .await;

            assert!(receive_request_res.is_ok());
        }

        let balance_of_call = build_message::<TokenRef>(token_address)
            .call(|token| token.balance_of(receiver_address));
        let balance = client
            .call_dry_run(&alice(), &balance_of_call, 0, None)
            .await
            .return_value();

        assert_eq!(balance, 0);

        Ok(())
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

    type CallResult<E> =
        Result<ink_e2e::CallResult<PolkadotConfig, DefaultEnvironment, Result<(), E>>, E>;
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

    async fn call_add_pair(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        token: AccountId,
        remote_token: [u8; 32],
    ) -> CallResult<MembraneError> {
        let add_pair_message = build_message::<MembraneRef>(membrane)
            .call(|membrane| membrane.add_pair(*token.as_ref(), remote_token));
        client
            .call_dry_run(caller, &add_pair_message, 0, None)
            .await
            .return_value()?;
        Ok(client
            .call(caller, add_pair_message, 0, None)
            .await
            .expect("Unexpected error."))
    }

    async fn call_send_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        token: AccountId,
        amount: u128,
        remote_address: [u8; 32],
    ) -> CallResult<MembraneError> {
        let send_request_message = build_message::<MembraneRef>(membrane)
            .call(|membrane| membrane.send_request(*token.as_ref(), amount, remote_address));
        client
            .call_dry_run(caller, &send_request_message, 0, None)
            .await
            .return_value()?;
        Ok(client
            .call(caller, send_request_message, 0, None)
            .await
            .expect("Unexpected error."))
    }

    async fn call_approve(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        spender: AccountId,
    ) -> CallResult<PSP22Error> {
        let approve_message =
            build_message::<TokenRef>(token).call(|token| token.approve(spender, amount));
        client
            .call_dry_run(caller, &approve_message, 0, None)
            .await
            .return_value()?;
        Ok(client
            .call(caller, approve_message, 0, None)
            .await
            .expect("Unexpected error."))
    }

    async fn call_transfer(
        client: &mut E2EClient,
        caller: &Keypair,
        token: AccountId,
        amount: u128,
        recipient: AccountId,
    ) -> CallResult<PSP22Error> {
        let transfer_message = build_message::<TokenRef>(token)
            .call(|token| token.transfer(recipient, amount, vec![]));
        client
            .call_dry_run(caller, &transfer_message, 0, None)
            .await
            .return_value()?;
        Ok(client
            .call(caller, transfer_message, 0, None)
            .await
            .expect("Unexpected error."))
    }

    async fn call_receive_request(
        client: &mut E2EClient,
        caller: &Keypair,
        membrane: AccountId,
        request_hash: Keccak256HashOutput,
        token: [u8; 32],
        amount: u128,
        receiver_address: [u8; 32],
        request_nonce: u128,
    ) -> CallResult<MembraneError> {
        let receive_request_message = build_message::<MembraneRef>(membrane).call(|membrane| {
            membrane.receive_request(request_hash, token, amount, receiver_address, request_nonce)
        });
        client
            .call_dry_run(caller, &receive_request_message, 0, None)
            .await
            .return_value()?;
        Ok(client
            .call(caller, receive_request_message, 0, None)
            .await
            .expect("Unexpected error."))
    }
}
