const hre = require("hardhat");

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

describe("MostBenchmark", function () {
  it(" deploy + estimate gas cost and successfully call sendRequest and receiveRequest.", async () => {
    const accounts = await hre.ethers.getSigners();

    const threshold = 5;
    const committeeSize = 8;

    let guardianKeys = accounts.slice(1, committeeSize + 1);
    let guardianAddresses = guardianKeys.map((x) => x.address);

    const Token = await hre.ethers.getContractFactory("Token");
    const token = await Token.deploy("10000", "18", "TestToken", "TEST");
    const tokenAddress = await token.getAddress();

    const Weth = await hre.ethers.getContractFactory("WETH9");
    const weth = await Weth.deploy();
    const wethAddress = await weth.getAddress();

    const Most = await hre.ethers.getContractFactory("Most");
    const most = await upgrades.deployProxy(
      Most,
      [guardianAddresses, threshold, accounts[0].address, wethAddress],
      {
        initializer: "initialize",
        kind: "uups",
      },
    );
    const mostAddress = await most.getAddress();

    // Easy way to get a "random" bytes32 value
    const azeroContract = getRandomAlephAccount(42);
    const azeroContract2 = getRandomAlephAccount(43);
    const tokenAddressBytes32 = addressToBytes32(tokenAddress);

    // Add pair of linked contracts
    await most.addPair(tokenAddressBytes32, azeroContract, { from: accounts[0] });
    await most.addPair(addressToBytes32(wethAddress), azeroContract2, {
      from: accounts[0],
    });
    await most.unpause();

    // Gas estimate for sendRequest

    // bytes32 "address" of account on Aleph
    const azeroAccount = getRandomAlephAccount(0);
    const amount = 1000;

    const gasEstimateApprove = await token.approve.estimateGas(
      mostAddress,
      amount,
      {
        from: accounts[0],
      },
    );

    console.log("Gas estimate for approve: ", Number(gasEstimateApprove));

    // Allow Most to spend tokens
    await token.approve(mostAddress, amount, {
      from: accounts[0],
    });

    const gasEstimateSend = await most.sendRequest.estimateGas(
      tokenAddressBytes32,
      amount,
      azeroAccount,
      { from: accounts[0] },
    );

    console.log("Gas estimate for sendRequest: ", Number(gasEstimateSend));

    await most.sendRequest(tokenAddressBytes32, amount, azeroAccount, {
      gas: gasEstimateSend,
      from: accounts[0],
    });

    const gasEstimateSendNative = await most.sendRequestNative.estimateGas(
      azeroAccount,
      { from: accounts[0], value: 2*amount },
    );

    console.log(
      "Gas estimate for sendRequestNative: ",
      Number(gasEstimateSendNative),
    );

    await most.sendRequestNative(azeroAccount, {
      gas: gasEstimateSendNative,
      from: accounts[0],
      value: 2*amount,
    });

    // Gas estimate for bridgeReceive
    const ethAccount = addressToBytes32(accounts[committeeSize + 1].address);
    const committeeId = 0;
    let nonce = 0;

    await benchmarkReceiveRequest(
      most,
      guardianKeys,
      committeeSize,
      committeeId,
      tokenAddressBytes32,
      amount,
      ethAccount,
      nonce,
      "token",
    );

    await benchmarkReceiveRequest(
      most,
      guardianKeys,
      committeeSize,
      committeeId,
      addressToBytes32(wethAddress),
      amount,
      ethAccount,
      ++nonce,
      "weth - no unwrap",
    );

    const zeroAddress = "0x0000000000000000000000000000000000000000000000000000000000000000";

    await benchmarkReceiveRequest(
      most,
      guardianKeys,
      committeeSize,
      committeeId,
      zeroAddress,
      amount,
      ethAccount,
      nonce,
      "weth - unwrap",
    );
  });
});

// function to benchmark the gas cost of receiveRequest with given parameters
async function benchmarkReceiveRequest(
  most,
  guardianKeys,
  committeeSize,
  committeeId,
  tokenAddressBytes32,
  amount,
  ethAccount,
  nonce,
  desc,
) {
  let requestHash = hre.ethers.solidityPackedKeccak256(
    ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
    [committeeId, tokenAddressBytes32, amount, ethAccount, nonce],
  );

  let gasEstimates = [...Array(committeeSize).keys()];
  for (let i = 0; i < committeeSize; i++) {
    gasEstimates[i] = Number(
      await most
        .connect(guardianKeys[i])
        .receiveRequest.estimateGas(
          requestHash,
          committeeId,
          tokenAddressBytes32,
          amount,
          ethAccount,
          nonce,
        ),
    );

    // Check if gas estimate is high enough
    await most
      .connect(guardianKeys[i])
      .receiveRequest(
        requestHash,
        committeeId,
        tokenAddressBytes32,
        amount,
        ethAccount,
        nonce,
        {
          gas: gasEstimates[i],
        },
      );
  }

  console.log("Gas estimates for receiveRequest (%s): ", desc, gasEstimates);

  // Sum gas estimates
  let sum = gasEstimates.reduce((a, b) => a + b, 0);
  console.log("Sum of gas estimates for receiveRequest (%s): ", desc, sum);
}
