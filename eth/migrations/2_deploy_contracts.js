const Web3 = require('web3');
const fs = require('fs');

const Membrane = artifacts.require("Membrane");
const TruffleConfig = require('../truffle-config.js');

module.exports = async function(deployer, network, accounts) {
    const config = TruffleConfig.networks[network];
    const opts = { gas: 1e6, from: accounts[0] };
    if (process.env.ACCOUNT_PASSWORD) {
        const web3 = new Web3(new Web3.providers.HttpProvider('http://' + config.host + ':' + config.port));
        console.log('Unlocking account: ' + accounts[0]);
        await web3.eth.personal.unlockAccount(accounts[0], process.env.ACCOUNT_PASSWORD, 86400);

        await deployer.deploy(Membrane, [accounts[0]], 1, opts);
        let instance = await Membrane.deployed();
       
        let addresses = {
            'membrane': instance.address
        };
        fs.writeFileSync('addresses.json', JSON.stringify(addresses));

    } else {
        throw new Error('provide ACCOUNT_PASSWORD ev var');
    }

};
