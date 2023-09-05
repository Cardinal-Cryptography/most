#!/bin/bash

set -e # exit immediately if any command has a non-zero exit status
# set -x # print all executed commands to the terminal
set -o pipefail #  prevents errors in a pipeline from being masked

source $(dirname "$0")/common.sh

# --- GLOBAL CONSTANTS

# --- FUNCTIONS

function flip() {
  local address=$(get_address flipper)
  cd "$CONTRACTS_PATH"/flipper
  cargo_contract call --url "$NODE" --contract "$address" --message flip --suri "$AUTHORITY_SEED" --skip-confirm
}

function get() {
  local address=$(get_address flipper)
  cd "$CONTRACTS_PATH"/flipper
  cargo_contract call --url "$NODE" --contract "$address" --message get --suri "$AUTHORITY_SEED" --dry-run --output-json | jq  -r '.data.Tuple.values' | jq '.[].Bool'
}

# --- RUN

if [ -z "$AUTHORITY_SEED" ]; then
  echo "\$AUTHORITY_SEED is empty"
  exit -1
fi

run_ink_dev

flip
get
