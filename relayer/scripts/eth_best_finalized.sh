#!/bin/bash

# require endpoint
ENDPOINT=${ENDPOINT:?"You have to specify the endpoint"}

FINALIZED_HEAD_NUMBER=$(curl -X POST -H "Content-Type: application/json" \
    --data '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["finalized",false],"id":"0"}' \
    -s ${ENDPOINT} | jq '.result.number' | tr -d '"')

# convert to decimal
echo $((FINALIZED_HEAD_NUMBER))
