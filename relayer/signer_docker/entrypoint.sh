#!/bin/bash

set -eou pipefail

# --- Prepare arguments

ARGS=(
  --azero-key=${AZERO_KEY}
  --eth-key=${ETH_KEY}
)

if [[ -n "${PORT}" ]]; then
  ARGS+=(--port=${PORT})
fi

# --- RUN

xargs ./signer "${ARGS[@]}"
