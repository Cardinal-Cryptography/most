#!/bin/bash

# set -x
set -eo pipefail

# --- GLOBAL CONSTANTS

ETH_ADDRESSES_FILE="/usr/local/contracts/eth_addresses.json"
AZERO_ADDRESSES_FILE="/usr/local/contracts/azero_addresses.json"
COMMON_ADDRESSES_FILE="/usr/local/contracts/common_addresses.json"

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

ADVISORY_ADDRESSES=${ADVISORY_ADDRESSES:-""}
AZERO_MOST_ADDRESS=${AZERO_MOST_ADDDRESS:-""}
ETH_MOST_ADDRESS=${ETH_MOST_ADDRESS:-""}

# --- Signer's CID
SIGNER_CID=${SIGNER_CID:-""}

# --- RELAYER ID from MY_POD_NAME coming from statefulset's pod, such as
# --- relayer-0, relayer-1 etc.
if [[ "${MY_POD_NAME}" =~ ^relayer-[0-9]+$ && "${RELAYER_ID}" == 0 ]]; then
  RELAYER_ID=$(echo "${MY_POD_NAME}" | cut -d- -f2)
  RELAYER_ID=$((RELAYER_ID+1))
fi

echo "RELAYER_ID=${RELAYER_ID}"

AZERO_MOST_METADATA=${AZERO_MOST_METADATA:-"/usr/local/most.json"}
ADVISORY_METADATA=${ADVISORY_METADATA:-"/usr/local/advisory.json"}
TOKEN_METADATA=${TOKEN_METADATA:-"/usr/local/token.json"}
ROUTER_METADATA=${ROUTER_METADATA:-"/usr/local/router.json"}

ARGS=(
  --name "guardian_${RELAYER_ID}"
  --advisory-contract-metadata=${ADVISORY_METADATA}
  --eth-node-http-url=${ETH_NETWORK}
  --azero-node-wss-url=${AZERO_NETWORK}
  --dev-account-index=${RELAYER_ID}
  --redis-node=${REDIS}
  --azero-contract-metadata=${AZERO_MOST_METADATA}
  --azero-ether-metadata=${TOKEN_METADATA}
  --router-metadata=${ROUTER_METADATA}
)

# --- Addresses can be passed as environment variables.
# --- If they are not, they should be present in the docker container.
if [[ -n "${ADVISORY_ADDRESSES}" ]]; then
  ARGS+=(--advisory-contract-addresses=${ADVISORY_ADDRESSES})
else
  if [[ -f "${AZERO_ADDRESSES_FILE}" ]]; then
    ARGS+=(--advisory-contract-addresses=$(get_address $AZERO_ADDRESSES_FILE advisory))
  else
    echo "! Advisory contract addresses are missing"
    exit 1
  fi
fi

if [[ -n "${AZERO_MOST_ADDRESS}" ]]; then
  ARGS+=(--azero-contract-address=${AZERO_MOST_ADDRESS})
else
  if [[ -f "${AZERO_ADDRESSES_FILE}" ]]; then
    ARGS+=(--azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most))
  else
    echo "! Azero most contract address is missing"
    exit 1
  fi
fi

if [[ -n "${ETH_MOST_ADDRESS}" ]]; then
  ARGS+=(--eth-contract-address=${ETH_MOST_ADDRESS})
else
  if [[ -f "${ETH_ADDRESSES_FILE}" ]]; then
    ARGS+=(--eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most))
  else
    echo "! Eth most contract address is missing"
    exit 1
  fi
fi

if [[ -n "${KEYSTORE_PATH}" ]]; then
  ARGS+=(--keystore-path=${KEYSTORE_PATH})
fi

if [[ -n "${DEV_MODE}" ]]; then
  ARGS+=(--dev)
fi

if [[ -n "${OVERRIDE_AZERO_CACHE}" ]]; then
  ARGS+=(--override-azero-cache)
fi

if [[ -n "${OVERRIDE_ETH_CACHE}" ]]; then
  ARGS+=(--override-eth-cache)
fi

if [[ -n "${AZERO_START_BLOCK}" ]]; then
  ARGS+=(--default-sync-from-block-azero=${AZERO_START_BLOCK})
fi

if [[ -n "${ETH_START_BLOCK}" ]]; then
  ARGS+=(--default-sync-from-block-eth=${ETH_START_BLOCK})
fi

if [[ -n "${SIGNER_CID}" ]]; then
  ARGS+=(--signer-cid=${SIGNER_CID})
fi

if [[ -n "${SYNC_STEP}" ]]; then
  ARGS+=(--sync-step=${SYNC_STEP})
fi

if [[ -n "${ETH_POLL_INTERVAL}" ]]; then
  ARGS+=(--eth-poll-interval=${ETH_POLL_INTERVAL})
fi

if [[ -n "${ETH_MIN_CONFIRMATIONS}" ]]; then
  ARGS+=(--eth-tx-min-confirmations=${ETH_MIN_CONFIRMATIONS})
fi

if [[ -n "${RUN_TRADER}" ]]; then
  ARGS+=(
    --run-trader-component
    --router-address=$(get_address $COMMON_ADDRESSES_FILE azero_router)
    --azero-ether-address=$(jq -r '.tokens[] | select(.[0] | endswith("ETH")) | .[2]' $AZERO_ADDRESSES_FILE)
    --azero-wrapped-azero-address=$(get_address $COMMON_ADDRESSES_FILE azero_wazero)
  )

  if [[ -n "${ETH_TO_AZERO_RELAYING_BUFFER}" ]]; then
    ARGS+=(--eth-to-azero-relaying-buffer=${ETH_TO_AZERO_RELAYING_BUFFER})
  fi

  if [[ -n "${BRIDGING_THRESHOLD}" ]]; then
    ARGS+=(--bridging-threshold=${BRIDGING_THRESHOLD})
  fi

  if [[ -n "${REWARD_WITHDRAWAL_THRESHOLD}" ]]; then
    ARGS+=(--reward-withdrawal-threshold=${REWARD_WITHDRAWAL_THRESHOLD})
  fi

fi

# --- RUN
xargs most-relayer "${ARGS[@]}"
