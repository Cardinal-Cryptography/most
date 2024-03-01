const { ethers, artifacts } = require("hardhat");
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

  // NOTE : TEMPorary before devnet is fixed and uses propere genesis that seeds these accounts with funds
  for (const to of signers.slice(1, 4)) {
    await signers[0].sendTransaction({
      to: to.address,
      value: ethers.parseEther("1.0"), // Send 1.0 ether
    });
  }

  // --- setup

  const Most = artifacts.require("Most");
  const most = await Most.at(contracts.most);

  // Add a pair
  const wethAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(contracts.weth),
    32,
  );
  const wethAddressBytesAzero = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroContracts.weth),
  );

  console.log(
    "Adding wETH token pair to Most:",
    contracts.weth,
    "=>",
    azeroContracts.weth,
  );

  const provider = new ethers.JsonRpcProvider(network.config.url);

  const signer0 = signers[1];
  const safeSdk0 = await createSafeInstance(signer0, contracts);

  console.log("safe owners", await safeSdk0.getOwners());
  console.log("signer0", signer0.address);

  let iface = await new ethers.Interface([
    "function addPair(bytes32 from, bytes32 to)",
  ]);
  let calldata = await iface.encodeFunctionData("addPair", [
    wethAddressBytes,
    wethAddressBytesAzero,
  ]);

  const safeTransactionData = {
    to: contracts.most,
    data: calldata,
    value: 0,
  };

  console.log("creating a Safe transaction:", safeTransactionData);

  const safeTransaction = await safeSdk0.createTransaction({
    transactions: [safeTransactionData],
  });
  const safeTxHash = await safeSdk0.getTransactionHash(safeTransaction);

  console.log("safeTxHash", safeTxHash);

  // on chain signatures
  await signSafeTransaction(safeSdk0, safeTxHash);

  const signer1 = signers[2];
  console.log("signer1", signer1.address);
  const safeSdk1 = await createSafeInstance(signer1, contracts);
  await signSafeTransaction(safeSdk1, safeTxHash);

  // execute safe tx
  await executeSafeTransaction(safeSdk1, safeTransaction);

  console.log(
    "Most now supports the token pair:",
    wethAddressBytes,
    "=>",
    await most.supportedPairs(wethAddressBytes),
  );

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
