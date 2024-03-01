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
  console.log("Signer", safeInstance, "is signing safe transaction", txHash);
  const approveTxResponse = await safeInstance.approveTransactionHash(txHash);
  await approveTxResponse.transactionResponse?.wait();
}

// executing safe tx (can be triggered by ANY account, not just one of the owners)
async function executeSafeTransaction(safeInstance, safeTransaction) {
  const executeTxResponse =
    await safeInstance.executeTransaction(safeTransaction);
  await executeTxResponse.transactionResponse?.wait();
}

async function addPair(ethContracts, azeroContracts, signers, mostInstance) {
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

  const safeSdk0 = await createSafeInstance(signers[1], contracts);

  console.log("safe owners", await safeSdk0.getOwners());

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

  console.log("Creating a Safe transaction:", safeTransactionData);

  const safeTransaction = await safeSdk0.createTransaction({
    transactions: [safeTransactionData],
  });
  const safeTxHash = await safeSdk0.getTransactionHash(safeTransaction);

  console.log("Safe transaction hash", safeTxHash);

  await signSafeTransaction(safeSdk0, safeTxHash);

  const safeSdk1 = await createSafeInstance(signers[2], contracts);
  await signSafeTransaction(safeSdk1, safeTxHash);

  await executeSafeTransaction(safeSdk1, safeTransaction);

  console.log(
    "Most now supports the token pair:",
    wethAddressBytes,
    "=>",
    await mostInstance.supportedPairs(wethAddressBytes),
  );
}

async function main() {
  const signers = await ethers.getSigners();
  const signer = signers[0];
  accounts = signers.map((s) => s.address);

  console.log("Using ", signer.address, "for signing transactions");

  // NOTE : TEMPorary before devnet is fixed and uses proper genesis that seeds these accounts with funds
  for (const to of signers.slice(1, 4)) {
    await signers[0].sendTransaction({
      to: to.address,
      value: ethers.parseEther("1.0"), // Send 1.0 ether
    });
  }

  // --- setup

  const Most = artifacts.require("Most");
  const most = await Most.at(contracts.most);

  // on other networks we do not have access to all of the governance keys
  if (["development", "bridgenet"].includes(network.name)) {
    await addPair(contracts, azeroContracts, signers, most);
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
