const { ethers, artifacts } = require("hardhat");
const { Keyring } = require("@polkadot/keyring");
const { u8aToHex } = require("@polkadot/util");

const contracts = require("../addresses.json");
const azeroContracts = require("../../azero/addresses.json");

async function transferOwnershipToGovernance(
  fromContract,
  governanceContract,
  signers
) {
  let iface = await new ethers.Interface(["function acceptOwnership()"]);
  let calldata = await iface.encodeFunctionData("acceptOwnership", []);
  let initialOwner = await fromContract.owner();
  console.log(
    "Transferring ownership: ",
    initialOwner,
    "=>",
    governanceContract.address
  );

  await fromContract.transferOwnership(governanceContract.address);
  // accept ownership by governance
  await governanceContract
    .connect(signers[1])
    .submitProposal(fromContract.address, calldata);
  console.log("Proposal submitted");
  console.log("Awaiting proposal ID...");
  let proposalId = await new Promise((resolve) => {
    governanceContract.on("ProposalSubmitted", (by, id) => {
      console.log(`Proposal ID: ${id}`);
      resolve(id);
    });
  });
  console.log("Signing proposal...");
  await governanceContract.connect(signers[2]).vote(proposalId);
  await governanceContract.connect(signers[3]).vote(proposalId);
  // execute proposal
  await governanceContract.connect(signers[1]).executeProposal(proposalId);
  console.log(`${initialOwner} ownership transferred successfully`);
}

async function main() {
  const signers = await ethers.getSigners();
  accounts = signers.map((s) => s.address);

  console.log("Using ", accounts[0], "as signer");

  // --- setup

  const Most = artifacts.require("Most");
  const most = await Most.at(contracts.most);

  // Add a pair
  const wethAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(contracts.weth9),
    32
  );
  const wethAddressBytesAzero = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroContracts.weth)
  );

  console.log(
    "Adding wETH token pair to Most:",
    contracts.weth9,
    "=>",
    azeroContracts.weth
  );

  await most.addPair(wethAddressBytes, wethAddressBytesAzero);

  // transfer governance ownership
  // we need "contractInstance" to be able to switch the signer
  const Governance = artifacts.require("Governance");
  let governanceInstance = await new ethers.Contract(
    contracts.governance,
    Governance.abi,
    signers[0]
    );
    governanceInstance.address = governanceInstance.runner.address;
    await transferOwnershipToGovernance(
      governanceInstance,
      governanceInstance,
    signers
    );
    
  // transfer most ownership
  let mostInstance = await new ethers.Contract(
    contracts.most,
    Most.abi,
    signers[0]
  );
  mostInstance.address = mostInstance.runner.address;
  await transferOwnershipToGovernance(
    mostInstance,
    governanceInstance,
    signers
  );

  const Migrations = artifacts.require("Migrations");
  const migrations = await Migrations.at(contracts.migrations);

  let lastCompletedMigration = await migrations.last_completed_migration();
  console.log("Updating migrations from", lastCompletedMigration, "to", 2);
  await migrations.setCompleted(2);

  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
