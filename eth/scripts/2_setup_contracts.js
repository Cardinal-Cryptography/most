const { ethers, artifacts, network } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");
const Safe = require("@safe-global/protocol-kit").default;
const { EthersAdapter } = require("@safe-global/protocol-kit");

const contracts = require("../addresses.json");
const azeroContracts = require("../../azero/addresses.json");

async function createSafeInstance(signer, contracts) {
  const ethAdapter = new EthersAdapter({
    ethers,
    signerOrProvider: signer,
  });
  const chainId = await ethAdapter.getChainId();
  const contractNetworks = {
    [chainId]: {
      safeSingletonAddress: contracts.gnosis.safeSingletonAddress,
      safeProxyFactoryAddress: contracts.gnosis.safeProxyFactoryAddress,
      multiSendAddress: contracts.gnosis.multiSendAddress,
      multiSendCallOnlyAddress: contracts.gnosis.multiSendCallOnlyAddress,
      fallbackHandlerAddress: contracts.gnosis.fallbackHandlerAddress,
      signMessageLibAddress: contracts.gnosis.signMessageLibAddress,
      createCallAddress: contracts.gnosis.createCallAddress,
      simulateTxAccessorAddress: contracts.gnosis.simulateTxAccessorAddress,
    },
  };

  return await Safe.create({
    ethAdapter: ethAdapter,
    safeAddress: contracts.gnosis.safe,
    contractNetworks,
  });
}

async function addTokenPair(
  ethTokenAddress,
  azeroTokenAddress,
  mostContract,
  safeInstances,
) {
  console.log(
    "Adding token pair to Most:",
    ethTokenAddress,
    "=>",
    azeroTokenAddress,
  );
  const ethTokenAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(ethTokenAddress),
    32,
  );
  const azeroTokenAddressBytes = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroTokenAddress),
  );
  let iface = await new ethers.Interface([
    "function addPair(bytes32 from, bytes32 to)",
  ]);
  let calldata = await iface.encodeFunctionData("addPair", [
    ethTokenAddressBytes,
    azeroTokenAddressBytes,
  ]);

  const safeTransactionData = {
    to: mostContract.address,
    data: calldata,
    value: 0,
  };

  console.log("creating a Safe transaction:", safeTransactionData);

  const safeTransaction = await safeInstances[0].createTransaction({
    transactions: [safeTransactionData],
  });
  const safeTxHash = await safeInstances[0].getTransactionHash(safeTransaction);

  console.log("safeTxHash", safeTxHash);

  for (const safeInstance of safeInstances) {
    await signSafeTransaction(safeInstance, safeTxHash);
  }

  // execute safe tx
  await executeSafeTransaction(safeInstances[0], safeTransaction);

  console.log(
    "Most now supports the token pair:",
    ethTokenAddressBytes,
    "=>",
    await mostContract.supportedPairs(ethTokenAddressBytes),
  );
}

// signing with on-chain signatures
async function signSafeTransaction(safeInstance, txHash) {
  const approveTxResponse = await safeInstance.approveTransactionHash(txHash);
  await approveTxResponse.transactionResponse?.wait();
}

async function executeSafeTransaction(safeInstance, safeTransaction) {
  const executeTxResponse =
    await safeInstance.executeTransaction(safeTransaction);
  await executeTxResponse.transactionResponse?.wait();
}

async function main() {
  const signers = await ethers.getSigners();
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  // --- setup

  const Most = artifacts.require("Most");
  const most = await Most.at(contracts.most);

  if (network.name == "development" || network.name == "bridgenet") {
    // NOTE : TEMPorary before devnet is fixed and uses propere genesis that seeds these accounts with funds
    for (const to of signers.slice(1, 4)) {
      await signers[0].sendTransaction({
        to: to.address,
        value: ethers.parseEther("1.0"), // Send 1.0 ether
      });
    }

    // --- provide some wETH and USDT to most contract
    const WETH = artifacts.require("WETH9");
    const weth = await WETH.at(contracts.weth);

    await weth.deposit({ value: 1000000000000000 });
    await weth.transfer(contracts.most, 1000000000000000);

    const Token = artifacts.require("Token");
    const usdt = await Token.at(contracts.usdt);
    await usdt.transfer(contracts.most, 1000000000000000);

    // --- add  pairs
    const signer0 = signers[1];
    const signer1 = signers[2];
    const safeSdk0 = await createSafeInstance(signer0, contracts);
    const safeSdk1 = await createSafeInstance(signer1, contracts);

    console.log("safe owners", await safeSdk0.getOwners());
    console.log("signer0", signer0.address);
    console.log("signer1", signer1.address);

    await addTokenPair(contracts.weth, azeroContracts.weth, most, [
      safeSdk0,
      safeSdk1,
    ]);

    await addTokenPair(contracts.usdt, azeroContracts.usdt, most, [
      safeSdk0,
      safeSdk1,
    ]);
  }

  // -- update migrations
  const Migrations = artifacts.require("Migrations");
  const migrations = await Migrations.at(contracts.migrations);

  let lastCompletedMigration = await migrations.last_completed_migration();
  console.log("Updating migrations from", lastCompletedMigration, "to", 2);
  await migrations.setCompleted(2);

  console.log("Done");
  // NOTE: neccessary because script hangs in CI
  process.exit(0);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
