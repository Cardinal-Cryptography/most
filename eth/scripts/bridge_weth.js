const { ethers, artifacts, network } = require("hardhat");

async function main() {
    const signers = await ethers.getSigners();
    accounts = signers.map((s) => s.address);

    console.log("Using ", accounts[0], "as signer");
    
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
