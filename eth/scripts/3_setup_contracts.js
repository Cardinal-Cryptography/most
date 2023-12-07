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
        'membrane': membraneEthereumAddress,
        'weth': wethEthereumAddress
    } = JSON.parse(
        fs.readFileSync(WETH_ETHEREUM_ADDRESS_PATH, 'utf8'),
    );

    const membrane = await hre.ethers.getContractAt('Membrane', membraneEthereumAddress);

    const keyring = new Keyring({ type: 'sr25519' });
    const wethAlephAddressBytes32 = keyring.decodeAddress(wethAlephAddress);
    const wethEthereumAddressBytes32 = hre.ethers.zeroPadValue(wethEthereumAddress, 32);

    await membrane.addPair(
        wethEthereumAddressBytes32,
        wethAlephAddressBytes32
    );
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
