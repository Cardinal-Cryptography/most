use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct TokenJson {
    pub symbol: String,
    pub address: String,
}

pub fn get_token_address_by_symbol(tokens: &[TokenJson], symbol: &str) -> String {
    tokens
        .iter()
        .find(|token| token.symbol == symbol)
        .unwrap()
        .address
        .clone()
}
