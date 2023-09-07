use ethers::contract::abigen;

abigen!(
    Flipper,
    r#"[
        event Flip(bool newValue)
    ]"#,
);
