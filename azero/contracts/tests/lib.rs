#![cfg_attr(not(feature = "std"), no_std, no_main)]
#[ink::contract]
mod tests {
    #[ink(storage)]
    pub struct Tests {}

    impl Tests {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self { }
        }

        #[ink(message)]
        pub fn test(&self) -> u32 {}
    }

    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e {
        use std::error::Error;
        use ink::primitives::AccountId;
        use ink_e2e::{account_id, build_message, AccountKeyring};
        use membrane::MembraneRef;
        use wrapped_token::TokenRef;

        fn guardian_ids() -> Vec<AccountId> {
            vec![
                account_id(AccountKeyring::Bob),
                account_id(AccountKeyring::Charlie),
                account_id(AccountKeyring::Dave),
                account_id(AccountKeyring::Eve),
                account_id(AccountKeyring::Ferdie),
            ]
        }

        #[ink_e2e::test]
        fn simple_deploy_works(mut client: ink_e2e::Client<C, E>) -> Result<(), Box<dyn Error>> {
            let membrane_constructor = MembraneRef::new(guardian_ids(), 3);
            let _membrane_address = client
                .instantiate("membrane", &ink_e2e::alice(), membrane_constructor, 0, None)
                .await
                .expect("instantiate failed")
                .account_id;
            Ok(())
        }

        #[ink_e2e::test]
        fn adding_pair_works(mut client: ink_e2e::Client<C, E>) -> Result<(), Box<dyn Error>> {
            let token_constructor = TokenRef::new(10000, None, None, 8);
            let token_address = client
                .instantiate("token", &ink_e2e::alice(), token_constructor, 0, None)
                .await
                .expect("instantiate failed")
                .account_id;

            let membrane_constructor = MembraneRef::new(guardian_ids(), 3);
            let membrane_address = client
                .instantiate("membrane", &ink_e2e::alice(), membrane_constructor, 0, None)
                .await
                .expect("instantiate failed")
                .account_id;

            let add_pair_bob = build_message::<MembraneRef>(membrane_address)
                .call(|membrane| membrane.add_pair(*token_address.as_ref(), [0x0; 32]));
            let add_pair_alice = build_message::<MembraneRef>(membrane_address)
                .call(|membrane| membrane.add_pair(*token_address.as_ref(), [0x0; 32]));
            
            let bob_res = client.call(
                    &ink_e2e::bob(),
                    add_pair_bob,
                    0,
                    None,
                ).await;
            let alice_res = client.call(
                    &ink_e2e::alice(),
                    add_pair_alice,
                    0,
                    None,
                ).await;

            assert!(bob_res.is_err());
            assert!(alice_res.is_ok());
            Ok(())
        }
    }
}
