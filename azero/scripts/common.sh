# --- GLOBAL CONSTANTS

CONTRACTS_PATH=$(pwd)/contracts
INK_DEV_IMAGE=public.ecr.aws/p6e8q1z1/ink-dev:1.0.0
NODE=ws://127.0.0.1:9944
AUTHORITY=5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
AUTHORITY_SEED=//Alice
ADDRESSES_FILE=$(pwd)/contracts/addresses.json

# --- FUNCTIONS

function run_ink_dev() {
  docker inspect ink_dev || docker run \
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

function get_address {
  local contract_name=$1
  cat $ADDRESSES_FILE | jq --raw-output ".$contract_name"
}
