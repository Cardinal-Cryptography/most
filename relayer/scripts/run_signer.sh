RELAYER_ID=${RELAYER_ID:-1}

RUST_LOG=info cargo run --bin signer -- --azero-key "//${RELAYER_ID}"
