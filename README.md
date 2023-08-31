# AlephZero Membrane (Guardian) Bridge

## Prerequisites

Have geth (Ethereum client) installed:

```bash
sudo add-apt-repository -y ppa:ethereum/ethereum
sudo apt-get update
sudo apt-get install ethereum
```
## Development

This section describes how set up a private PoA ethereum network.

`Node1` is specified in the genesis as the authority (signer) for block creation.
`Node2` is an archiver and has it's ws/http ports exposed to the host for e.g. using it with `remix`
Both nodes connect to a common `bootnode`.

To run the setup start by initializing the nodes:

```bash
geth init --datadir bootstrap genesis.json && \
geth init --datadir node1 genesis.json && \
geth init --datadir node2 genesis.json
```

Now you can start the bridge services:

```bash
docker-compose -f bridge.compose.yml up
```

To clean up:

```bash
rm -rf bootstrap/geth node1/geth node2/geth
```

Re-initialzie the nodes and run them again from a fresh genesis block.
