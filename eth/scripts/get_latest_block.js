const fs = require("node:fs");
const { network, ethers } = require("hardhat");

async function main() {
  const latestBlock = await ethers.provider.getBlock("latest");
  const block_latest = {
    number: latestBlock.number,
  };
  fs.writeFileSync("block_latest.json", JSON.stringify(block_latest));
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
