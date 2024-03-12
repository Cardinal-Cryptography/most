#!/bin/bash

readonly SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

echo "Running drink tests: ${SCRIPT_DIR}"

cd ${SCRIPT_DIR}/../contracts/drink-tests && cargo test -- --nocapture
