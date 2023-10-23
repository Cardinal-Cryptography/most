#!/bin/bash

PORT=8545

# Start a development network
npx ganache-cli -p ${PORT} -h 127.0.0.1 -d --gasLimit 8000000 > .ganache_dev.log &

# Run tests
npx truffle test --network testing
TEST_EXIT_CODE=$?

# Stop the development network
kill $(lsof -t -i:${PORT})

# If the tests failed, print the log
if [ ${TEST_EXIT_CODE} -ne 0 ]; then
    echo "Tests failed. Printing ganache logs:"
    cat .ganache_dev.log
fi

# Exit with the same code as the tests
exit ${TEST_EXIT_CODE}
