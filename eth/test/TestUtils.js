const hre = require("hardhat");

// function that converts an address to a bytes32
function addressToBytes32(address) {
    return hre.ethers.zeroPadValue(address, 32);
}

// function that converts a bytes32 to an address
function bytes32ToAddress(bytes32) {
    return "0x" + bytes32.slice(26, 66);
}

function getRandomAlephAccount(number) {
    return hre.ethers.solidityPackedKeccak256(["uint"], [number]);
}

function ethToWei(ethAmount) {
    return hre.ethers.parseEther(ethAmount.toString());
}

module.exports = {
    addressToBytes32,
    bytes32ToAddress,
    ethToWei,
    getRandomAlephAccount
};
