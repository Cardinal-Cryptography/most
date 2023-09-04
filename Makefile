clean:
	rm -rf bootstrap/geth node1/geth node2/geth 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/db/full 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/db/full

bootstrap:
	docker run -v ./0x7f938fd203fcabc449312661ab1d36b7fdb45118:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && docker run -v ./0x129b9daee478e7bc5edada471982e31fa7705622:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && docker run -v ./0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json

bridgenet:
	docker-compose -f bridge.compose.yml up

watch:
	cd contracts/azero/flipper && cargo watch -s 'cargo contract check' -c

release:
	cd contracts/azero/flipper &&  cargo contract build --release
