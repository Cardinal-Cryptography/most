#!/bin/bash

# set -x
set -eo pipefail

# --- GLOBAL CONSTANTS

ETH_ADDRESSES_FILE=$(pwd)/../eth/addresses.json
AZERO_ADDRESSES_FILE=$(pwd)/../azero/contracts/addresses.json

# --- FUNCTIONS

function get_address {
  local addresses_file=$1
  local contract_name=$2
  cat $addresses_file | jq --raw-output ".$contract_name"
}

# --- RUN

RUST_LOG=debug AZERO_CONTRACT_ADDRESS=$(get_address $AZERO_ADDRESSES_FILE flipper) ETH_CONTRACT_ADDRESS=$(get_address $ETH_ADDRESSES_FILE flipper) cargo run
