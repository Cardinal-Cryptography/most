#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod flipper {
    use ink::{codegen::EmitEvent, reflect::ContractEventBase};

    #[ink(storage)]
    pub struct Flipper {
        flip: bool,
        flop: bool,
    }

    pub type Event = <Flipper as ContractEventBase>::Type;

    #[ink(event)]
    #[derive(Debug)]
    pub struct Flip {
        value: bool,
    }

    #[ink(event)]
    #[derive(Debug)]
    pub struct Flop {
        value: bool,
    }

    impl Flipper {
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                flip: false,
                flop: false,
            }
        }

        #[ink(message)]
        pub fn flip(&mut self) {
            self.flip = !self.flip;
            Self::emit_event(self.env(), Event::Flip(Flip { value: self.flip }));
        }

        #[ink(message)]
        pub fn flop(&mut self) {
            self.flop = !self.flop;
            Self::emit_event(self.env(), Event::Flop(Flop { value: self.flop }));
        }

        #[ink(message)]
        pub fn get(&self) -> (bool, bool) {
            (self.flip, self.flop)
        }

        fn emit_event<EE>(emitter: EE, event: Event)
        where
            EE: EmitEvent<Self>,
        {
            emitter.emit_event(event);
        }
    }
}
