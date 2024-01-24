[![Built with ink!](https://raw.githubusercontent.com/paritytech/ink/master/.images/badge_flat.svg)](https://github.com/paritytech/ink)

# Aleph Zero MOST (Guardian) Bridge

Contains a guardian-based implementation of a token bridge between Ethereum and Aleph Zero.

## Contents

* `azero` - Aleph Zero contracts
  * `azero/contracts/most` - the main bridge contract
  * `azero/contracts/governance` - a contract for making governance decisions about the bridge contracts on Aleph Zero
  * `azero/contracts/gas-price-oracle` - a contract for informing the guardians about Ethereum gas prices
  * `azero/contracts/token` - a PSP22 token implementation for the tokens created by the bridge
* `eth` - Ethereum contracts
  * `eth/contracts/Most.sol` - the main bridge contract
  * `eth/contracts/Governance.sol` - a contract for making governance decisions about the bridge contracts on Ethereum
  * `eth/contracts/WETH9.sol` - a wrapped ethereum implementation for testing purposes
  * `eth/contract/Token.sol` - an ERC20 token implementation for testing purposes
* `relayer` - the component doing the actual bridging between the chains

## Prerequisites

Have docker and docker-compose installed.

## Running the local development environment

To run a local development environment, run:

```bash
make bridge
```

This will start a local development environment with the following components:
* Aleph Zero Development chain
* Ethereum PoS Development chain
* Bridge contracts deployed on both chains
* Redis instance
* Relayer process

## Running local development chains

To run the development chain without deploying the contracts or running the relayer, run:

```bash
make devnet-eth
```
or
```bash
make devnet-azero
```

## Cleaning up

To stop the networks (and redis) and clean up:

```bash
make clean
```

### Check eth nodes connectivity

get a shell into running container:

```bash
docker exec -it eth_bootnode "geth attach /root/.ethereum/geth.ipc"
```

attach console to the geth process:

```bash
geth attach /root/.ethereum/geth.ipc
```

Check the connected peers, confirm these are the correct nodes:

```bash
net.peerCount
admin.peers
```

## Deploying contracts

To deploy the contracts on the EVM chain:

```bash
make deploy-eth
```

This command will deploy the contracts and print out the contract addresses.

Now, open a Hardhat console:

```bash
cd eth && npx hardhat console --network development
```

Interact with the deployed contracts instance:

```javascript
let Most = await ethers.getContractFactory("Most")
let most = most.attach("insert contract address here")
let accounts = await ethers.getSigners();

await most.isGuardian(accounts[0])
await most.isGuardian(accounts[1])
```

To deploy the contracts on the Aleph Zero chain:

```bash
make deploy-azero
```

## Running the relayer process

```bash
make run-relayer
```
