#!/bin/bash

# set -x
set -eo pipefail

# --- GLOBAL CONSTANTS

ETH_ADDRESSES_FILE="/usr/local/eth_addresses.json"
AZERO_ADDRESSES_FILE="/usr/local/azero_addresses.json"

# --- FUNCTIONS

function get_address {
  local addresses_file=$1
  local contract_name=$2
  cat $addresses_file | jq --raw-output ".$contract_name"
}

# --- ARGS

ETH_NETWORK=${ETH_NETWORK:-"ws://127.0.0.1:8546"}
AZERO_NETWORK=${AZERO_NETWORK:-"ws://127.0.0.1:9944"}
REDIS=${REDIS:-"redis://127.0.0.1:6379"}

KEYSTORE_PATH=${KEYSTORE_PATH:-""}
RELAYER_ID=${RELAYER_ID:-0}

ARGS=(
  --name "guardian_${RELAYER_ID}"
  --azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most)
  --eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most)
  --eth-node-wss-url=${ETH_NETWORK}
  --azero-node-wss-url=${AZERO_NETWORK}
  --dev-account-index=${RELAYER_ID}
  --redis-node=${REDIS}
  --rust-log=info
)

if [[ -n "$KEYSTORE_PATH" ]]; then
  ARGS+=(--keystore-path=${KEYSTORE_PATH})
fi

# --- RUN
xargs most-relayer "${ARGS[@]}"

