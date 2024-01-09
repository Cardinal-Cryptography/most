const hre = require("hardhat");

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

describe("MostBenchmark", function () {
  it(" deploy + estimate gas cost and successfully call sendRequest and receiveRequest.", async () => {
    const accounts = await hre.ethers.getSigners();

    let guardianKeys = accounts.slice(1, 9);
    let guardianAddresses = guardianKeys.map((x) => x.address);
    let threshold = 5;

    const Token = await hre.ethers.getContractFactory("Token");
    const token = await Token.deploy("10000", "TestToken", "TEST");
    const tokenAddress = await token.getAddress();

    const Most = await hre.ethers.getContractFactory("Most");
    const most = await upgrades.deployProxy(
      Most,
      [guardianAddresses, 5, accounts[0].address],
      {
        initializer: "initialize",
        kind: "uups",
      },
    );
    const mostAddress = await most.getAddress();

    // Easy way to get a "random" bytes32 value
    let azContract = getRandomAlephAccount(42);
    let tokenAddressBytes32 = addressToBytes32(tokenAddress);

    // Add pair of linked contracts
    await most.addPair(tokenAddressBytes32, azContract, { from: accounts[0] });

    // Gas estimate for sendRequest

    // bytes32 "address" of account on Aleph
    let azAccount = getRandomAlephAccount(0);

    // Allow Most to spend tokens
    await token.approve(mostAddress, 1000, {
      from: accounts[0],
    });

    const gasEstimateSend = await most.sendRequest.estimateGas(
      tokenAddressBytes32,
      1000,
      azAccount,
      { from: accounts[0] },
    );

    console.log("Gas estimate for sendRequest: ", Number(gasEstimateSend));

    await most.sendRequest(tokenAddressBytes32, 1000, azAccount, {
      gas: gasEstimateSend,
      from: accounts[0],
    });

    // Gas estimate for bridgeReceive
    let ethAccount = addressToBytes32(accounts[9].address);
    let requestHash = hre.ethers.solidityPackedKeccak256(
      ["bytes32", "uint256", "bytes32", "uint256"],
      [tokenAddressBytes32, 1000, ethAccount, 1],
    );

    // Estimate gas for each signature
    let gasEstimates = [...Array(threshold).keys()];
    for (let i = 0; i < threshold; i++) {
      gasEstimates[i] = Number(
        await most
          .connect(guardianKeys[i])
          .receiveRequest.estimateGas(
            requestHash,
            tokenAddressBytes32,
            1000,
            ethAccount,
            1,
          ),
      );

      // Check if gas estimate is high enough
      await most
        .connect(guardianKeys[i])
        .receiveRequest(requestHash, tokenAddressBytes32, 1000, ethAccount, 1, {
          gas: gasEstimates[i],
        });
    }

    console.log("Gas estimates for receiveRequest: ", gasEstimates);

    // Sum gas estimates
    let sum = gasEstimates.reduce((a, b) => a + b, 0);
    console.log("Sum of gas estimates for receiveRequest: ", sum);
  });
});
