use std::env;

#[derive(Default, Debug, Clone)]
pub struct Config {
    pub log_level: String,
    pub eth_node_wss_url: String,
    // TODO : pass abi meta
    pub eth_contract_address: String,
    pub azero_node_wss_url: String,
    pub azero_sudo_seed: String,
    pub azero_contract_metadata: String,
    pub azero_contract_address: String,

    pub eth_last_known_block: usize,
    pub azero_last_known_block: usize,
}

pub trait Load {
    // Static method signature; `Self` refers to the implementor type
    fn load() -> Self;
}

impl Load for Config {
    fn load() -> Config {
        Config {
            log_level: get_env_var("LOG_LEVEL", Some(String::from("info"))),
            azero_node_wss_url: get_env_var(
                "AZERO_WSS_URL",
                Some(String::from("ws://127.0.0.1:9944")),
            ),
            azero_last_known_block: get_env_var("AZERO_LAST_KNOWN_BLOCK", Some(String::from("0")))
                .parse()
                .expect("Can't parse as int"),
            azero_contract_metadata: get_env_var("FLIPPER_METADATA", Some(String::from("/home/filip/CloudStation/aleph/membrane-bridge/azero/contracts/flipper/target/ink/flipper.json"))),
            azero_contract_address: get_env_var("AZERO_CONTRACT_ADDRESS", None),
            azero_sudo_seed: get_env_var("AZERO_SUDO_SEED", Some(String::from("//Alice"))),
            eth_node_wss_url: get_env_var("ETH_WSS_URL", Some(String::from("ws://127.0.0.1:8546"))),                        
            eth_contract_address: get_env_var("ETH_CONTRACT_ADDRESS", None),
            eth_last_known_block: get_env_var("ETH_LAST_KNOWN_BLOCK", Some(String::from("0")))
                .parse()
                .expect("Can't parse as int"),

        }
    }
}

fn get_env_var(var: &str, default: Option<String>) -> String {
    match env::var(var) {
        Ok(v) => v,
        Err(_) => match default {
            None => panic!("Missing ENV variable: {var} not defined in environment"),
            Some(d) => d,
        },
    }
}
