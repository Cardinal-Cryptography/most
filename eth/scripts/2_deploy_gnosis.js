const Safe = require('@safe-global/protocol-kit').default;
const { SafeFactory, EthersAdapter } = require('@safe-global/protocol-kit');
const { network, ethers } = require("hardhat");

async function main() {

    switch (network.name) {
    case 'development':
        const provider = await ethers.provider;
        const safeOwner = await provider.getSigner(0)
        const ethAdapter = new EthersAdapter({
            ethers,
            signerOrProvider: safeOwner
        })

        // deploy gnosis contracts
        const GnosisSafeProxyFactory = await ethers.getContractFactory("GnosisSafeProxyFactory");
        console.log("Deploying GnosisSafeProxyFactory...");
        let gnosisSafeProxyFactory = await GnosisSafeProxyFactory.deploy();
        console.log("GnosisSafeProxyFactory deployed to:", gnosisSafeProxyFactory.target);

        const Singleton = await ethers.getContractFactory("Singleton");
        console.log("Deploying Singleton...");
        let singleton = await Singleton.deploy();
        console.log("Singleton deployed to:", singleton.target);

        const GnosisSafeProxy = await ethers.getContractFactory("GnosisSafeProxy");
        console.log("Deploying GnosisSafeProxy...");
        let gnosisSafeProxy = await GnosisSafeProxy.deploy(singleton.target);
        console.log("GnosisSafeProxy deployed to:", gnosisSafeProxy.target);

        const GnosisSafe = await ethers.getContractFactory("GnosisSafe");
        console.log("Deploying GnosisSafe...");
        const gnosisSafe = await GnosisSafe.deploy();
        console.log("GnosisSafe deployed to:", gnosisSafe.target);

        const MultiSend = await ethers.getContractFactory("MultiSend");
        console.log("Deploying MultiSend...");
        const multiSend = await MultiSend.deploy();
        console.log("MultiSend deployed to:", multiSend.target);

        const chainId = await ethAdapter.getChainId()

        // await some confirmations, else it gets finicky
        await gnosisSafeProxyFactory.deploymentTransaction().wait(1);
        await gnosisSafeProxy.deploymentTransaction().wait(1);
        await singleton.deploymentTransaction().wait(1);

        const contractNetworks  = {
            [chainId]: {
                multiSendAddress: multiSend.target,
                safeSingletonAddress:  singleton.target,
                safeProxy:  gnosisSafeProxy.target,
                safeProxyFactoryAddress:  gnosisSafeProxyFactory.target,
                safeMasterCopyAddress: gnosisSafe.target,
            }

        }

        console.log("Gnosis Safe contracts", contractNetworks);

        const safeFactory = await SafeFactory.create({ ethAdapter, contractNetworks });

        // create new Safe

        const safeSdk = await Safe.create({ ethAdapter, safeAddress: gnosisSafe.target, contractNetworks })


        break;
        // TODO: for other networks augment hardhat config with the addresses of the already deployed gnosis contracts
    default:
        console.log(`Unknown network ${network.name}`);
        process.exit(-1);
    }

}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
