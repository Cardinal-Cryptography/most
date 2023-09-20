.PHONY: clean
clean:
	rm -rf 0x129b9daee478e7bc5edada471982e31fa7705622/geth 0x129b9daee478e7bc5edada471982e31fa7705622/geth.ipc 0x129b9daee478e7bc5edada471982e31fa7705622/history 0x129b9daee478e7bc5edada471982e31fa7705622/jwt.hex 0x7f938fd203fcabc449312661ab1d36b7fdb45118/geth 0x7f938fd203fcabc449312661ab1d36b7fdb45118/geth.ipc 0x7f938fd203fcabc449312661ab1d36b7fdb45118/history 0x7f938fd203fcabc449312661ab1d36b7fdb45118/jwt.hex 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/geth 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/history 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/jwt.hex 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/db 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/network 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/backup-stash 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/db 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/network 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/backup-stash 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/db 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/network 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json && echo "Done"

.PHONY: bootstrap
bootstrap:
	cp eth_genesis.json 0x7f938fd203fcabc449312661ab1d36b7fdb45118/genesis.json && docker run -v ./0x7f938fd203fcabc449312661ab1d36b7fdb45118:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp eth_genesis.json 0x129b9daee478e7bc5edada471982e31fa7705622/genesis.json && docker run -v ./0x129b9daee478e7bc5edada471982e31fa7705622:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp eth_genesis.json 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/genesis.json && docker run -v ./0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp azero_chainspec.json 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json && cp azero_chainspec.json 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json && cp azero_chainspec.json 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json && echo "Done"

.PHONY: bridge
bridge:
	docker-compose -f bridge.compose.yml up

.PHONY: compile-eth
compile-eth:
	cd eth && truffle compile

.PHONY: deploy-eth
deploy-eth:
	cd eth && truffle migrate --reset

.PHONY: watch-azero
watch-azero:
	cd azero/contracts/flipper && cargo watch -s 'cargo contract check' -c

.PHONY: deploy-azero
deploy-azero:
	cd azero/contracts/ && ./scripts/deploy.sh

.PHONY: watch-relayer
watch-relayer:
	cd relayer && cargo watch -s 'cargo clippy' -c
