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

const COMMISSION_PER_DIX_MILLE = 30;
const MINIMUM_TRANSFER_AMOUNT_USD = 50;

async function deployEightGuardianMostFixture() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  const Most = await ethers.getContractFactory("Most");
  const most = await upgrades.deployProxy(
    Most,
    [
      accounts.slice(1, 9),
      5,
      COMMISSION_PER_DIX_MILLE,
      MINIMUM_TRANSFER_AMOUNT_USD,
      accounts[0],
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  const mostAddress = await most.getAddress();

  const Token = await ethers.getContractFactory("Token");
  const token = await Token.deploy(
    "10000000000000000000000000",
    "Shittoken",
    "SHIT",
  );
  const tokenAddressBytes32 = addressToBytes32(await token.getAddress());

  const usdt = await Token.deploy(
    "12000000000000000000000000",
    "Tether",
    "USDT",
  );
  const usdtAddressBytes32 = addressToBytes32(await usdt.getAddress());

  await most.setUSDT(usdtAddressBytes32);
  return {
    most,
    token,
    tokenAddressBytes32,
    usdtAddressBytes32,
    mostAddress,
  };
}

module.exports = {
  addressToBytes32,
  bytes32ToAddress,
  ethToWei,
  getRandomAlephAccount,
  deployEightGuardianMostFixture,
  COMMISSION_PER_DIX_MILLE,
  MINIMUM_TRANSFER_AMOUNT_USD,
};
