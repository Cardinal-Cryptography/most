const fs = require("node:fs");
const hre = require("hardhat");
const { Keyring } = require('@polkadot/keyring');

async function main() {
    const WETH_ALEPH_ADDRESS_PATH = '../azero/addresses.json';
    const WETH_ETHEREUM_ADDRESS_PATH = 'addresses.json';

    const {
        'weth': wethAlephAddress,
        ..._
    } = JSON.parse(
        fs.readFileSync(WETH_ALEPH_ADDRESS_PATH, 'utf8'),
    );

    const {
        'governance': governanceEthereumAddress,
        'membrane': membraneEthereumAddress,
        'weth': wethEthereumAddress
    } = JSON.parse(
        fs.readFileSync(WETH_ETHEREUM_ADDRESS_PATH, 'utf8'),
    );

    const governance = await hre.ethers.getContractAt('Governance', governanceEthereumAddress);
    const membrane = await hre.ethers.getContractAt('Membrane', membraneEthereumAddress);

    await governance.setOwner(governanceEthereumAddress);
    await membrane.setOwner(governanceEthereumAddress);
    console.log('wethAleph: ', wethAlephAddress);
    console.log('wethEthereum: ', wethEthereumAddress);

    const keyring = new Keyring({ type: 'sr25519'});
    const wethAlephAddressBytes32 = keyring.decodeAddress(wethAlephAddress);
    console.log('wethAlephBytes32: ', wethAlephAddressBytes32);
    const wethEthereumAddressBytes32 = hre.ethers.zeroPadValue(wethEthereumAddress, 32);
    console.log('wethEthereumBytes32: ', wethEthereumAddressBytes32);
    console.log('randomAlephBytes32: ', hre.ethers.solidityPackedKeccak256(["uint"], [5]));

    await membrane.addPair(
        wethEthereumAddressBytes32,
        wethAlephAddressBytes32
    );
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
