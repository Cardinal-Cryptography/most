#!/bin/bash

readonly INK_DEV_IMAGE="public.ecr.aws/p6e8q1z1/ink-dev:2.1.0"
readonly SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

## Builds all contracts and generates code for contract interaction. Run in a container.
docker run --rm -it \
    --name ink-dev \
    -v "$SCRIPT_DIR/..":/code \
    $INK_DEV_IMAGE \
    bash -c "chmod +x ./scripts/prepare_rust_wrappers.sh && ./scripts/prepare_rust_wrappers.sh"
    