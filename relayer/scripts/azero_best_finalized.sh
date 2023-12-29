#!/bin/bash

set -eo pipefail

# require endpoint
ENDPOINT=${ENDPOINT:?"You have to specify the endpoint"}

FINALIZED_HEAD_HASH=$(curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "chain_getFinalizedHead", "params":[]}' ${ENDPOINT} | jq -r '.result')
FINALIZED_HEAD_NUMBER=$(curl -s -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "chain_getHeader", "params":["'"$FINALIZED_HEAD_HASH"'"]}' ${ENDPOINT} | jq -r '.result.number')

# convert to decimal
echo $((FINALIZED_HEAD_NUMBER))
