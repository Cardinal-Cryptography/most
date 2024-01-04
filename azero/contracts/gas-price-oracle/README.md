# Ethereum gas oracle contracts

The corresponding parts are:
1. `trait` -- you should use the trait to integrate the oracle in another contract. Both `test-contract` and `contract` implement it.
2. `test-contract` -- you should use that for local and dev deployments. This oracle does not require a feeder.
3. `contract` -- the actual oracle contract. Expected to be used in production or close-to-production deployments. This oracle requires a feeder.