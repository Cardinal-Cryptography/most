const Migrations = artifacts.require("Migrations");

module.exports = async function (deployer, network, accounts) {
    const opts = { gas: 1e6, from: accounts[0] };
    deployer.deploy(Migrations, opts);
};
