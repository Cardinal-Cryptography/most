const fs = require("node:fs");
const Safe = require("@safe-global/protocol-kit").default;
const { SafeFactory, EthersAdapter } = require("@safe-global/protocol-kit");
const { network, ethers } = require("hardhat");

async function main() {
  const provider = new ethers.JsonRpcProvider(network.config.url);
  console.log("Deploying GnosisSafe to:", network.name);

  const signers = await ethers.getSigners();
  const signer = signers[0];
  console.log("Using", signer, "as the transaction signer");

  const ethAdapter = new EthersAdapter({
    ethers,
    signerOrProvider: signer,
  });

  const safeAccountConfig = {
    owners: network.config.deploymentConfig.governanceIds,
    threshold: network.config.deploymentConfig.governanceThreshold,
  };

  console.log("GnosisSafe deployment config:", safeAccountConfig);

  let safeFactory,
    safe,
    safeAddress,
    address = null;

  switch (network.name) {
    case "development":
    case "bridgenet":

      // deploy gnosis contracts
      const GnosisSafeProxyFactory = await ethers.getContractFactory(
        "GnosisSafeProxyFactory",
      );
      console.log("Deploying GnosisSafeProxyFactory...");
      let gnosisSafeProxyFactory = await GnosisSafeProxyFactory.deploy();
      console.log(
        "GnosisSafeProxyFactory deployed to:",
        gnosisSafeProxyFactory.target,
      );

      const Singleton = await ethers.getContractFactory("Singleton");
      console.log("Deploying Singleton...");
      let singleton = await Singleton.deploy();
      console.log("Singleton deployed to:", singleton.target);

      const GnosisSafe = await ethers.getContractFactory("GnosisSafe");
      console.log("Deploying GnosisSafe...");
      const gnosisSafe = await GnosisSafe.deploy();
      console.log("GnosisSafe deployed to:", gnosisSafe.target);

      const MultiSend = await ethers.getContractFactory("MultiSend");
      console.log("Deploying MultiSend...");
      const multiSend = await MultiSend.deploy();
      console.log("MultiSend deployed to:", multiSend.target);

      const MultiSendCallOnly =
        await ethers.getContractFactory("MultiSendCallOnly");
      console.log("Deploying MultiSendCallOnly...");
      const multiSendCallOnly = await MultiSendCallOnly.deploy();
      console.log("MultiSendCallOnly deployed to:", multiSendCallOnly.target);

      const FallbackManager =
        await ethers.getContractFactory("FallbackManager");
      console.log("Deploying FallbackManager...");
      const fallbackManager = await FallbackManager.deploy();
      console.log("FallbackManager deployed to:", fallbackManager.target);

      const SignMessageLib = await ethers.getContractFactory("SignMessageLib");
      console.log("Deploying SignMessageLib...");
      const signMessageLib = await SignMessageLib.deploy();
      console.log("SignMessageLib deployed to:", signMessageLib.target);

      const CreateCall = await ethers.getContractFactory("CreateCall");
      console.log("Deploying CreateCall...");
      const createCall = await CreateCall.deploy();
      console.log("CreateCall deployed to:", createCall.target);

      const SimulateTxAccessor =
        await ethers.getContractFactory("SimulateTxAccessor");
      console.log("Deploying SimulateTxAccessor...");
      const simulateTxAccessor = await SimulateTxAccessor.deploy();
      console.log("SimulateTxAccessor deployed to:", simulateTxAccessor.target);

      const chainId = await ethAdapter.getChainId();

      // await 1 block confirmations, else it gets finicky
      await singleton.deploymentTransaction().wait(1);
      await gnosisSafeProxyFactory.deploymentTransaction().wait(1);
      await multiSend.deploymentTransaction().wait(1);
      await multiSendCallOnly.deploymentTransaction().wait(1);
      await fallbackManager.deploymentTransaction().wait(1);
      await signMessageLib.deploymentTransaction().wait(1);
      await createCall.deploymentTransaction().wait(1);
      await simulateTxAccessor.deploymentTransaction().wait(1);

      const contractNetworks = {
        [chainId]: {
          safeSingletonAddress: gnosisSafe.target,
          safeProxyFactoryAddress: gnosisSafeProxyFactory.target,
          multiSendAddress: multiSend.target,
          multiSendCallOnlyAddress: multiSendCallOnly.target,
          fallbackHandlerAddress: fallbackManager.target,
          signMessageLibAddress: signMessageLib.target,
          createCallAddress: createCall.target,
          simulateTxAccessorAddress: simulateTxAccessor.target,
        },
      };

      console.log("Gnosis Safe contracts", contractNetworks);

      safeFactory = await SafeFactory.create({
        ethAdapter,
        contractNetworks,
        isL1SafeSingleton: false, // forces the use of SafeL2.sol contract that emits events but consumes more more gas
      });

      safe = await safeFactory.deploySafe({ safeAccountConfig });
      safeAddress = await safe.getAddress();
      console.log("GnosisSafe address:", safeAddress);

      // --- spit addresses

      addresses = {
        safe: safeAddress,
        safeSingletonAddress: gnosisSafe.target,
        safeProxyFactoryAddress: gnosisSafeProxyFactory.target,
        multiSendAddress: multiSend.target,
        multiSendCallOnlyAddress: multiSendCallOnly.target,
        fallbackHandlerAddress: fallbackManager.target,
        signMessageLibAddress: signMessageLib.target,
        createCallAddress: createCall.target,
        simulateTxAccessorAddress: simulateTxAccessor.target,
      };

      fs.writeFileSync("addresses.json", JSON.stringify(addresses));
      break;

    case "sepolia":
      safeFactory = await SafeFactory.create({
        ethAdapter: ethAdapter,
        isL1SafeSingleton: false,
      });

      safe = await safeFactory.deploySafe({ safeAccountConfig });
      safeAddress = await safe.getAddress();
      console.log("GnosisSafe address:", safeAddress);

      // --- spit addresses

      addresses = {
        safe: safeAddress,
      };

      fs.writeFileSync("addresses.json", JSON.stringify(addresses));
      break;

    default:
      console.log(`Uknown network name ${network.name}`);
      process.exit(-1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
