const { ethers, artifacts } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");

const contracts = require("../addresses.json");
const azeroContracts = require("../../azero/addresses.json");

// async function transferOwnershipToGovernance(
//   fromContract,
//   governanceContract,
//   governanceSigners,
// ) {
//   let iface = await new ethers.Interface(["function acceptOwnership()"]);
//   let calldata = await iface.encodeFunctionData("acceptOwnership", []);
//   let initialOwner = await fromContract.owner();
//   console.log(
//     "Transferring ownership: ",
//     initialOwner,
//     "=>",
//     governanceContract.target,
//   );

//     await fromContract.transferOwnership(governanceContract.target);
//   await governanceContract
//     .connect(governanceSigners[0])
//     .submitProposal(fromContract.target, calldata);
//   console.log("Proposal submitted");
//   console.log("Awaiting proposal ID...");

//     let proposalId = await new Promise((resolve) => {
//     governanceContract.on("ProposalSubmitted", (by, id) => {
//       console.log(`Proposal ID: ${id}`);
//       resolve(id);
//     });
//   });
//   console.log("Signing proposal...");
//   for (const member of governanceSigners.slice(1)) {
//     await governanceContract.connect(member).vote(proposalId);
//   }
//   await governanceContract
//     .connect(governanceSigners[0])
//     .executeProposal(proposalId);
//   console.log(`${initialOwner} ownership transferred successfully`);
// }

async function createSafeInstance(signer, contracts) {
    const ethAdapter = new EthersAdapter({
        ethers,
        signerOrProvider: signer
    })

    const chainId = await ethAdapter.getChainId()
    const contractNetworks  = {
        [chainId]: {
            safeSingletonAddress: contracts.safeSingletonAddress,
            safeProxyFactoryAddress: contracts.safeProxyFactoryAddress
            multiSendAddress: contracts.multiSendAddress,
            multiSendCallOnlyAddress: contracts.multiSendCallOnlyAddress,
            fallbackHandlerAddress: contracts.fallbackHandlerAddress,
            signMessageLibAddress: contracts.signMessageLibAddress,
            createCallAddress: contracts.createCallAddress,
            simulateTxAccessorAddress: contracts.simulateTxAccessorAddress
        }
    }

    return await Safe.create({ ethAdapter: ethAdapter, contracts.safe, contractNetworks})
}

// on-chain signature
async function signSafeTransaction(safeInstance, txHash) {

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

    // IN-PROGRESS: add pair via a governance Safe action
    // await most.addPair(wethAddressBytes, wethAddressBytesAzero);

    const signer0 = await provider.getSigner(0);
    const safeSdk0 = await createSafeInstance(signer0, contracts);

    let iface = await new ethers.Interface(["function addPair()"]);
    let calldata = await iface.encodeFunctionData("addPair", [wethAddressBytes, wethAddressBytesAzero]);
    const safeTransactionData = {
        to: contracts.most,
        data: calldata,
        // value: 0
    }
    const safeTransaction = await safeSdk0.createTransaction({ transactions: [safeTransactionData] });
    const safeTxHash = await safeSdk0.getTransactionHash(safeTransaction);

    // on chain signature
    const approveTxResponse = await safeSdk0.approveTransactionHash(safeTxHash)
    await approveTxResponse.transactionResponse?.wait()

    console.log("approve tx response:", approveTxResponse);

    const signer1 = await provider.getSigner(1);
    const safeSdk1 = await createSafeInstance(signer1, contracts);

    const approveTxResponse = await safeSdk0.approveTransactionHash(safeTxHash)
    await approveTxResponse.transactionResponse?.wait()

    console.log("approve tx response:", approveTxResponse);

    // execute safe tx

    const executeTxResponse = await safeSdk2.executeTransaction(safeTransaction)
    await executeTxResponse.transactionResponse?.wait()

   console.log("execute tx response:", executeTxResponse);





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
