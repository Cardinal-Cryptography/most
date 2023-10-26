NETWORK ?= development
AZERO_ENV ?= dev

.PHONY: help
help: # Show help for each of the Makefile recipes.
	@grep -E '^[a-zA-Z0-9 -]+:.*#'  Makefile | sort | while read -r l; do printf "\033[1;32m$$(echo $$l | cut -f 1 -d':')\033[00m:$$(echo $$l | cut -f 2- -d'#')\n"; done

.PHONY: clean-azero
clean-azero: # Remove azero node data
clean-azero:
	cd devnet-azero && rm -rf \
	5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/db \
	5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chains/a0dnet1/network \
	5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/backup-stash \
	5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json \
	5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/db \
	5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chains/a0dnet1/network \
	5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/backup-stash \
	5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json \
	5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash \
	5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/db \
	5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chains/a0dnet1/network \
	5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/backup-stash \
	5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json \
	&& echo "Done azero clean"

.PHONY: clean-eth
clean-eth: # Remove eth node data
clean-eth:
	cd devnet-eth && ./clean.sh && echo "Done eth clean"

.PHONY: clean
clean: # Remove all node data
clean: clean-azero clean-eth

.PHONY: bootstrap-azero
bootstrap-azero: # Bootstrap the node data
bootstrap-azero:
	cd devnet-azero && \
	cp azero_chainspec.json 5D34dL5prEUaGNQtPPZ3yN5Y6BnkfXunKXXz6fo7ZJbLwRRH/chainspec.json && \
	cp azero_chainspec.json 5GBNeWRhZc2jXu7D55rBimKYDk8PGk8itRYFTPfC8RJLKG5o/chainspec.json && \
	cp azero_chainspec.json 5Dfis6XL8J2P6JHUnUtArnFWndn62SydeP8ee8sG2ky9nfm9/chainspec.json && echo "Done"

.PHONY: devnet-azero
devnet-azero: # Run azero devnet
devnet-azero: bootstrap-azero
	docker-compose -f ./devnet-azero/devnet-azero-compose.yml up -d

.PHONY: devnet-eth
devnet-eth: # Run eth devnet
devnet-eth:
	docker-compose -f ./devnet-eth/devnet-eth-compose.yml up -d

.PHONY: bridge
bridge: # Run whole local bridge setup
bridge: devnet-azero devnet-eth

.PHONY: eth-deps
eth-deps: # Install eth dependencies
eth-deps:
	cd eth && npm install

.PHONY: watch-eth
watch-eth: # watcher on the eth contracts
watch-eth:
	cd eth && npm run watch

.PHONY: compile-eth
compile-eth: # Compile eth contracts
compile-eth:
	cd eth && npx hardhat compile

.PHONY: deploy-eth
deploy-eth: # Deploy eth contracts
deploy-eth:
	cd eth && \
	npx hardhat run --network $(NETWORK) scripts/1_initial_migration.js && \
	npx hardhat run --network $(NETWORK) scripts/2_deploy_contracts.js

.PHONY: membrane-builder
membrane-builder: # Build an image in which contracts can be built
membrane-builder:
	docker build -t membrane-builder -f docker/membrane_builder.dockerfile .

.PHONY: compile-azero-docker
compile-azero-docker: # Compile azero contracts in docker
compile-azero-docker: azero-deps membrane-builder
	docker run --rm --network host \
		--volume "$(shell pwd)":/code \
		--workdir /code \
		--name membrane-builder \
		membrane-builder \
		make compile-azero

.PHONY: deploy-azero-docker
deploy-azero-docker: # Deploy azero contracts compiling in docker
deploy-azero-docker: azero-deps compile-azero-docker
	cd azero && npm run deploy

.PHONY: azero-deps
azero-deps: # Install azero dependencies
azero-deps:
	cd azero && npm install

.PHONY: watch-azero
watch-azero: # watch azero contracts and generate artifacts
watch-azero:
	cd azero && npm run watch

.PHONY: compile-azero
compile-azero: # compile azero contracts and generate artifacts
compile-azero:
	cd azero && npm run compile

.PHONY: deploy-azero
deploy-azero: # Deploy azero contracts
deploy-azero:
	cd azero && npm run compile && npm run deploy

.PHONY: watch-relayer
watch-relayer:
	cd relayer && cargo watch -s 'cargo clippy' -c

.PHONY: run-relayer
run-relayer: # Run the relayer
run-relayer:
	cd relayer && ./scripts/run.sh

.PHONY: test-solidity
test-solidity: # Run solidity tests
test-solidity: eth-deps
	cd eth && npx hardhat test
