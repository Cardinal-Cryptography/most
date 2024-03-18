#!/bin/bash

# set -x
set -eo pipefail

# --- GLOBAL CONSTANTS

ETH_ADDRESSES_FILE=$(pwd)/../eth/addresses.json
AZERO_ADDRESSES_FILE=$(pwd)/../azero/addresses.json

# --- FUNCTIONS

function get_address {
  local addresses_file=$1
  local contract_name=$2
  cat $addresses_file | jq --raw-output ".$contract_name"
}

# --- ARGS

ETH_NETWORK=${ETH_NETWORK:-"http://127.0.0.1:8545"}
AZERO_NETWORK=${AZERO_NETWORK:-"ws://127.0.0.1:9944"}

RELAYER_ID=${RELAYER_ID:-0}

ARGS=(
  run --bin relayer -- --name "guardian_${RELAYER_ID}" \
  --advisory-contract-addresses=$(get_address $AZERO_ADDRESSES_FILE advisory) \
  --azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most) \
  --eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most) \
  --eth-node-http-url=${ETH_NETWORK} \
  --azero-node-wss-url=${AZERO_NETWORK} \
  --dev-account-index=${RELAYER_ID} \
  --default-sync-from-block-eth=0 \
  --default-sync-from-block-azero=0 \
  --override-azero-cache \
  --override-eth-cache \
  --dev
)

if [[ -n "${SIGNER_CID}" ]]; then
  ARGS+=(--signer-cid=${SIGNER_CID})
fi

# --- RUN

export RUST_LOG=info,aleph-client=warn

cargo "${ARGS[@]}"
