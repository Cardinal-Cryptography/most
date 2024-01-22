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

ETH_NETWORK=${ETH_NETWORK:-"http://127.0.0.1:8545"}
AZERO_NETWORK=${AZERO_NETWORK:-"ws://127.0.0.1:9944"}
REDIS=${REDIS:-"redis://127.0.0.1:6379"}

KEYSTORE_PATH=${KEYSTORE_PATH:-""}
RELAYER_ID=${RELAYER_ID:-0}

# --- RELAYER ID from MY_POD_NAME coming from statefulset's pod, such as
# --- relayer-0, relayer-1 etc.
if [[ "${MY_POD_NAME}" =~ ^relayer-[0-9]+$ && "${RELAYER_ID}" == 0 ]]; then
  RELAYER_ID=$(echo "${MY_POD_NAME}" | cut -d- -f2)
  RELAYER_ID=$((RELAYER_ID+1))
fi

echo "RELAYER_ID=${RELAYER_ID}"

AZERO_MOST_METADATA=${AZERO_MOST_METADATA:-"/usr/local/most.json"}

ARGS=(
  --name "guardian_${RELAYER_ID}"
  --azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most)
  --eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most)
  --eth-node-http-url=${ETH_NETWORK}
  --azero-node-wss-url=${AZERO_NETWORK}
  --dev-account-index=${RELAYER_ID}
  --redis-node=${REDIS}
  --rust-log=info
  --azero-contract-metadata=${AZERO_MOST_METADATA}
)

if [[ -n "${KEYSTORE_PATH}" ]]; then
  ARGS+=(--keystore-path=${KEYSTORE_PATH})
fi

if [[ -n "${DEV_MODE}" ]]; then
  ARGS+=(--dev)
fi

if [[ -n "${AZERO_START_BLOCK}" ]]; then
  ARGS+=(--default-sync-from-block-azero=${AZERO_START_BLOCK})
fi

# --- RUN
xargs most-relayer "${ARGS[@]}"
