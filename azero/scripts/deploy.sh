#!/bin/bash

# set -x
set -eo pipefail

CONTRACTS_PATH=$(pwd)/contracts
INK_DEV_IMAGE=public.ecr.aws/p6e8q1z1/ink-dev:1.0.0
NODE=ws://127.0.0.1:9944
AUTHORITY=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
AUTHORITY_SEED=//Alice

# --- FUNCTIONS

function run_ink_dev() {
  docker start ink_dev || docker run \
                                 --network host \
                                 -v "${PWD}:/sources" \
                                 --name ink_dev \
                                 --detach \
                                 --rm $INK_DEV_IMAGE sleep 1d
}

function cargo_contract() {
  contract_dir=$(basename "${PWD}")
  docker exec \
         -w "/sources/contracts/$contract_dir" \
         -e RUST_LOG=info \
         -e CARGO_TARGET_DIR=/tmp/ \
         ink_dev cargo contract "$@"
}

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
