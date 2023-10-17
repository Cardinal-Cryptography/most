// function that converts an address to a bytes32
function addressToBytes32(address) {
    return web3.utils.padLeft(address, 64);
}

// function that converts a bytes32 to an address
function bytes32ToAddress(bytes32) {
    return web3.utils.toChecksumAddress("0x" + bytes32.slice(26, 66));
}

module.exports = {
    addressToBytes32,
    bytes32ToAddress
};