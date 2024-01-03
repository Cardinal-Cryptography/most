#!/bin/bash

# This script is used to start the bridge node with stdout redirected to stderr and replaced localhost address
# These changes are needed for the node to be compatible with ink_e2e tests
docker compose -f ../../../devnet-azero/devnet-azero-compose.yml up | sed 's/0\.0\.0\.0/127.0.0.1/' 1>&2
