RELAYER_ID=${RELAYER_ID:-1}

ETH_KEYS=(
    ["1"]="f0ecd1edf6c8bd1249e3e89935c433f891b30571b22cacf22bae0cd974f6c349"
    ["2"]="5ade60ea7c8b4413ff558807ac703d9b77705c9fc70e8d7150e8eea3f71a3bc9"
    ["3"]="ec3a4667d2182f119b09a97772a523ecea714dddd7649701d013c10a4b4ba771"
)
ETH_KEY=${ETH_KEYS[${RELAYER_ID}]}

RUST_LOG=info cargo run --bin signer -- --azero-key "//${RELAYER_ID}" --eth-key "${ETH_KEY}"
