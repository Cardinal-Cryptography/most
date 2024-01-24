[![Built with ink!](https://raw.githubusercontent.com/paritytech/ink/master/.images/badge_flat.svg)](https://github.com/paritytech/ink)

![Most logo](MOST_logo_dark.svg "Most logo")

# AlephZero MOST<sup>*</sup> (Guardian) Bridge

Bring your assets to Aleph Zero and benefit from its scalable privacy-enhancing ecosystem.

<sup>_\*'bridge' in Slavic languages._</sup>

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

To deploy the contracts on the azero chain:

```bash
make deploy-azero
```

## Running the relayer process

```bash
make run-relayer
```
