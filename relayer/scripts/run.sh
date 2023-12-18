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

# --- NETWORKS

#ETH_NETWORK="https://rpc-eth-bridgenet.dev.azero.dev"
#AZERO_NETWORK="wss://ws-fe-bridgenet.dev.azero.dev"

ETH_NETWORK=${ETH_NETWORK:-"ws://127.0.0.1:8546"}
AZERO_NETWORK=${AZERO_NETWORK:-"ws://127.0.0.1:9944"}

# --- RELAYER ID

RELAYER_ID=${RELAYER_ID:-0}

# --- RUN

cargo run -- --rust-log=info --name "guardian1" --azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most) --eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most) \
  --eth-node-wss-url=${ETH_NETWORK} --azero-node-wss-url=${AZERO_NETWORK} --dev-account-index=${RELAYER_ID}
