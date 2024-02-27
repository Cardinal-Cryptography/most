[![Built with ink!](https://raw.githubusercontent.com/paritytech/ink/master/.images/badge_flat.svg)](https://github.com/paritytech/ink)

<img src="most_logo.png" alt="Most logo">

# Aleph Zero MOST<sup>*</sup> (Guardian) Bridge

Bring your assets to Aleph Zero and benefit from its scalable privacy-enhancing ecosystem.

<sup>_\*'bridge' in Slavic languages._</sup>

## Contents

* `azero` - Aleph Zero contracts
  * `azero/contracts/most` - the main bridge contract
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

## Running a relayer with a signer locally

First, run the signer:

```bash
cd relayer
RELAYER_ID=1 scripts/run_signer.sh
```

The signer will output the CID on which it is running, but you should also be able to just use 2 (meaning host) as the CID.

Next, run the relayer, passing the signer CID to connect to:

```bash
# in most/relayer
SIGNER_CID=2 scripts/run_relayer.sh
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

## Running the relayer processes

```bash
make run-relayers
```

## Verifying deployed contracts against source code

Given a deployed (by us) Aleph Zero contract with some code hash `C` it's possible to check that the contract has been
produced from a certain version of the source code in this repo (say a given commit). To do so:

1. `git checkout $COMMIT`
2. `make print-azero-codehashes`
3. Find the contract in question in the list and ensure that the printed code hash is the same as `C`

The contracts will be deployed using the same docker image as the one used for this procedure, which smooths out
indeterminism in ink! contract compilation.
