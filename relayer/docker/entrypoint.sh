#!/usr/bin/env bash
set -euo pipefail

# load env variables from a file if ENV_FILE is set
if [[ -n "${ENV_FILE:-}" ]] && [[ -f "${ENV_FILE}" ]]; then
  set -o allexport
  source ${ENV_FILE}
  set +o allexport
fi


AZERO_CONTRACT_ADDRESS=${AZERO_CONTRACT_ADDRESS:?'azero contract address must be present'}
ETH_CONTRACT_ADDRESS=${ETH_CONTRACT_ADDRESS:?"eth contract address must be present"}

AZERO_CONTRACT_METADATA=${AZERO_CONTRACT_METADATA:-"/data/flipper.json"}
AZERO_LAST_KNOWN_BLOCK=${AZERO_LAST_KNOWN_BLOCK:-"0"}
AZERO_NODE_WSS_URL=${AZERO_NODE_WSS_URL:-"ws://127.0.0.1:9944"}
AZERO_SUDO_SEED=${AZERO_SUDO_SEED:-"//Alice"}

ETH_KEYSTORE_PASSWORD=${ETH_KEYSTORE_PASSWORD:-"chaos555"}
ETH_KEYSTORE_PATH=${ETH_KEYSTORE_PATH:-"/data/eth/0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/keystore/f2f0930c3b7bdf1734ee173272bd8cdc0a08f038"}
ETH_LAST_KNOWN_BLOCK=${ETH_LAST_KNOWN_BLOCK:-"0"}
ETH_NODE_WSS_URL=${ETH_NODE_WSS_URL:-"ws://127.0.0.1:8546"}

RUST_LOG=${RUST_LOG:-"info"}

ARGS=(
  --azero_contract_address "${AZERO_CONTRACT_ADDRESS}"
  --eth_contract_address "${ETH_CONTRACT_ADDRESS}"
  --azero_contract_metadata "${AZERO_CONTRACT_METADATA}"
  --azero_last_known_block "${AZERO_LAST_KNOWN_BLOCK}"
  --azero_node_wss_url "${AZERO_NODE_WSS_URL}"
  --azero_sudo_seed "${AZERO_SUDO_SEED}"
  --eth_keystore_password "${ETH_KEYSTORE_PASSWORD}"
  --eth_keystore_path "${ETH_KEYSTORE_PATH}"
  --eth_last_known_block "${ETH_LAST_KNOWN_BLOCK}"
  --eth_node_wss_url "${ETH_NODE_WSS_URL}"
)

relayer "${ARGS[@]}"
