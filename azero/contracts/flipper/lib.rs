#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod flipper {
    use ink::codegen::EmitEvent;
    use ink::reflect::ContractEventBase;

    #[ink(storage)]
    pub struct Flipper {
        value: bool,
    }

    pub type Event = <Flipper as ContractEventBase>::Type;

    #[ink(event)]
    #[derive(Debug)]
    pub struct Flip {
        new_value: bool,
    }

    impl Flipper {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self { value: false }
        }

        #[ink(message)]
        pub fn flip(&mut self) {
            self.value = !self.value;

            Self::emit_event(
                self.env(),
                Event::Flip(Flip {
                    new_value: self.value,
                }),
            );
        }

        #[ink(message)]
        pub fn get(&self) -> bool {
            self.value
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
