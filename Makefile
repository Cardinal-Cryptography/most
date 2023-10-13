NETWORK ?= development
NODE ?= ws://127.0.0.1:9944

.PHONY: help
help: # Show help for each of the Makefile recipes.
	@grep -E '^[a-zA-Z0-9 -]+:.*#'  Makefile | sort | while read -r l; do printf "\033[1;32m$$(echo $$l | cut -f 1 -d':')\033[00m:$$(echo $$l | cut -f 2- -d'#')\n"; done

.PHONY: clean
clean: # Remove node data
clean:
	rm -rf 0x129b9daee478e7bc5edada471982e31fa7705622/geth 0x129b9daee478e7bc5edada471982e31fa7705622/geth.ipc 0x129b9daee478e7bc5edada471982e31fa7705622/history 0x129b9daee478e7bc5edada471982e31fa7705622/jwt.hex 0x7f938fd203fcabc449312661ab1d36b7fdb45118/geth 0x7f938fd203fcabc449312661ab1d36b7fdb45118/geth.ipc 0x7f938fd203fcabc449312661ab1d36b7fdb45118/history 0x7f938fd203fcabc449312661ab1d36b7fdb45118/jwt.hex 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/geth 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/history 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/jwt.hex 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/db 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/network 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/backup-stash 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/db 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/network 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/backup-stash 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/db 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/network 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json && echo "Done"

.PHONY: stop-ink-dev
stop-ink-dev: # Stop the helper container
stop-ink-dev:
	docker kill ink_dev || true

.PHONY: bootstrap
bootstrap: # Bootstrap the node data
bootstrap:
	cp eth_genesis.json 0x7f938fd203fcabc449312661ab1d36b7fdb45118/genesis.json && docker run -u "${UID}:${GID}" -v ./0x7f938fd203fcabc449312661ab1d36b7fdb45118:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp eth_genesis.json 0x129b9daee478e7bc5edada471982e31fa7705622/genesis.json && docker run -u "${UID}:${GID}" -v ./0x129b9daee478e7bc5edada471982e31fa7705622:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp eth_genesis.json 0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038/genesis.json && docker run -u "${UID}:${GID}" -v ./0xf2f0930c3b7bdf1734ee173272bd8cdc0a08f038:/root/.ethereum ethereum/client-go:release-1.12 init --datadir /root/.ethereum /root/.ethereum/genesis.json && cp azero_chainspec.json 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json && cp azero_chainspec.json 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json && cp azero_chainspec.json 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json && echo "Done"

.PHONY: bridge
bridge: # Run eth and aleph nodes in docker
bridge:
	docker-compose -f bridge.compose.yml up

.PHONY: eth-deps
eth-deps: # Install eth dependencies
eth-deps:
	npm install -g truffle
	cd eth && npm install

.PHONY: compile-eth
compile-eth: # Compile eth contracts
compile-eth:
	cd eth && truffle compile

.PHONY: deploy-eth
deploy-eth: # Deploy eth contracts
deploy-eth:
	cd eth && truffle migrate --network $(NETWORK)

.PHONY: watch-azero
watch-azero:
	cd azero/contracts/flipper && cargo watch -s 'cargo contract check' -c

.PHONY: deploy-azero
deploy-azero: # Deploy azero contracts
deploy-azero:
	cd azero && NODE=$(NODE) ./scripts/deploy.sh

.PHONY: watch-relayer
watch-relayer:
	cd relayer && cargo watch -s 'cargo clippy' -c

.PHONY: run-relayer
run-relayer: # Run the relayer
run-relayer:
	cd relayer && ./scripts/run.sh
