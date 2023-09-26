#!/bin/bash

# set -x
set -eo pipefail

source $(dirname "$0")/common.sh

# --- GLOBAL CONSTANTS

# --- FUNCTIONS

# --- RUN

run_ink_dev

# compile & deploy contracts

cd "$CONTRACTS_PATH"/flipper
cargo_contract build --release
FLIPPER_CODE_HASH=$(cargo_contract upload --url "$NODE" --suri "$AUTHORITY_SEED" --output-json --skip-confirm  | jq -s . | jq -r '.[1].code_hash')
FLIPPER=$(cargo_contract instantiate --url "$NODE" --constructor new --suri "$AUTHORITY_SEED" --skip-confirm --output-json | jq -r '.contract')

# spit adresses to a JSON file
cd "$CONTRACTS_PATH"

jq -n \
   --arg flipper "$FLIPPER" \
   --arg flipper_code_hash "$FLIPPER_CODE_HASH" \
   '{
      flipper: $flipper,
      flipper_code_hash: $flipper_code_hash
    }' > addresses.json

cat addresses.json

exit $?
