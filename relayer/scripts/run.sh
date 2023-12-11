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

# --- RUN

cargo run -- --rust-log=info --name "guardian1" --azero-contract-address=$(get_address $AZERO_ADDRESSES_FILE most) --eth-contract-address=$(get_address $ETH_ADDRESSES_FILE most)
