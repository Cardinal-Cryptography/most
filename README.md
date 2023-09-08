# AlephZero Membrane (Guardian) Bridge

## Prerequisites

Have docker and docker-compose installed.

## Development

This section describes how set up a private PoA ethereum network.

`Node1` service is specified in the genesis as the authority (signer) for block creation.
`Node2` is an archiver and has it's ws/http ports exposed to the host for e.g. using it with `remix`
Both nodes connect to a common `bootnode`.

To run the setup start by bootstrapping the nodes to the common genesis:

```bash
make bootstrap
```

Now you can start the bridge services:

```bash
make bridgenet
```

To clean up:

```bash
make clean
```
Re-initialize the nodes and run them again from a fresh genesis block.

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
cd eth/
truffle migrate --reset
```

Open a console:

```bash
truffle console 
```
Interact with the deployed contracts instance:

```javascript
let flipper = await Flipper.deployed()
flipper.flip()
flipper.value()
```
