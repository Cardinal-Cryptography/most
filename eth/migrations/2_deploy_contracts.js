const fs = require('node:fs');

const Flipper = artifacts.require('Flipper');
const TruffleConfig = require('../truffle-config.js');

module.exports = async function (deployer, network, accounts) {
	const options = {gas: 1e6, from: accounts[0]};
	await deployer.deploy(Flipper, options);
	const instance = await Flipper.deployed();

	const addresses = {
		flipper: instance.address,
	};
	fs.writeFileSync('addresses.json', JSON.stringify(addresses));
};
