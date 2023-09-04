# AlephZero Membrane (Guardian) Bridge

## Prerequisites

Have docker and docker-compose installed.

## Development

This section describes how set up a private PoA ethereum network.

`Node1` is specified in the genesis as the authority (signer) for block creation.
`Node2` is an archiver and has it's ws/http ports exposed to the host for e.g. using it with `remix`
Both nodes connect to a common `bootnode`.

To run the setup start by bootstrapping the nodes to the common genesis:

```bash
docker run -v ./0x7f938fd203fcabc449312661ab1d36b7fdb45118:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json &&

docker run -v ./0x129b9daee478e7bc5edada471982e31fa7705622:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json &&

docker run -v ./0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json
```

Now you can start the bridge services:

```bash
docker-compose -f bridge.compose.yml up
```

To clean up:

```bash
rm -rf bootstrap/geth node1/geth node2/geth
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
