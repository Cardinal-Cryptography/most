#!/bin/bash

# Runs ink-wrapper in order to generate Rust wrapper for selected contracts.
# Copies contracts' wasm files to the drink-tests resources directory.
# Requires that contracts had been built before running this script.

readonly SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

declare -a CONTRACTS=(
    "advisory"
    "most"
    "token"
    "migrations"
    "wrapped_azero"
)

# Process gas price oracle contract. Requires special handling due to a different directory structure and name.
function gas_price_oracle() {
    echo "Wrapping gas price oracle" ;
    ink-wrapper --metadata $SCRIPT_DIR/../contracts/gas-price-oracle/contract/target/ink/oracle.json  \
        --wasm-path ../../resources/gas_price_oracle.wasm | rustfmt --edition 2021 > $SCRIPT_DIR/../contracts/drink-tests/src/wrappers/gas_price_oracle.rs ;
    echo "Copying contract gas price oracle";
    cp $SCRIPT_DIR/../contracts/gas-price-oracle/contract/target/ink/oracle.wasm  $SCRIPT_DIR/../contracts/drink-tests/resources/gas_price_oracle.wasm ;
}

function copy_contract() {
    for c in ${CONTRACTS[@]}; do
        echo "Copying contract for $c" ;
        cp $SCRIPT_DIR/../contracts/$c/target/ink/$c.wasm  $SCRIPT_DIR/../contracts/drink-tests/resources/ ;
    done
}

function compile_contracts() {
    for c in ${CONTRACTS[@]}; do
        echo "Compiling $c" ;
        cargo contract build --release --manifest-path $SCRIPT_DIR/../contracts/$c/Cargo.toml
    done
}

function wrap_contracts() {
    for c in ${CONTRACTS[@]}; do
        echo "Wrapping $c" ;
        ink-wrapper --metadata $SCRIPT_DIR/../contracts/$c/target/ink/$c.json  \
            --wasm-path ../../resources/$c.wasm | rustfmt --edition 2021 > $SCRIPT_DIR/../contracts/drink-tests/src/wrappers/$c.rs ;
    done
    gas_price_oracle
}

function run() {
    compile_contracts
    wrap_contracts
    copy_contract
}

run
