const Safe = require('@safe-global/protocol-kit').default;
const { SafeFactory, EthersAdapter } = require('@safe-global/protocol-kit');
const { network, ethers } = require("hardhat");

async function main() {

    switch (network.name) {
    case 'development':

        const provider = new ethers.JsonRpcProvider(network.config.url);
        const accounts = network.config.accounts;

        var index = 0;
        const wallet0 = ethers.Wallet.fromPhrase(accounts.mnemonic, accounts.path + `/${index}`);
        const signer0 = new ethers.Wallet(wallet0.privateKey, provider)

        index = 1;
        const wallet1 = ethers.Wallet.fromPhrase(accounts.mnemonic, accounts.path + `/${index}`);
        const signer1 = new ethers.Wallet(wallet1.privateKey, provider)

        index = 2;
        const wallet2 = ethers.Wallet.fromPhrase(accounts.mnemonic, accounts.path + `/${index}`);
        const signer2 = new ethers.Wallet(wallet2.privateKey, provider)

        // const signers = await ethers.getSigners();
        // const accounts = signers.map((s) => s.address);
        // const governance_accounts = accounts.slice(1,4);

        // const provider = await ethers.provider;
        // const safeOwner = await provider.getSigner(0);
        const ethAdapter = new EthersAdapter({
            ethers,
            signerOrProvider: signer0
        })

        // deploy gnosis contracts
        const GnosisSafeProxyFactory = await ethers.getContractFactory("GnosisSafeProxyFactory");
        // console.log("GnosisSafeProxyFactory", GnosisSafeProxyFactory);
        // process.exit(0);
        console.log("Deploying GnosisSafeProxyFactory...");
        let gnosisSafeProxyFactory = await GnosisSafeProxyFactory.deploy();
        console.log("GnosisSafeProxyFactory deployed to:", gnosisSafeProxyFactory.target);

        const Singleton = await ethers.getContractFactory("Singleton");
        console.log("Deploying Singleton...");
        let singleton = await Singleton.deploy();
        console.log("Singleton deployed to:", singleton.target);

        // const GnosisSafeProxy = await ethers.getContractFactory("GnosisSafeProxy");
        // console.log("Deploying GnosisSafeProxy...");
        // let gnosisSafeProxy = await GnosisSafeProxy.deploy(singleton.target);
        // console.log("GnosisSafeProxy deployed to:", gnosisSafeProxy.target);

        const GnosisSafe = await ethers.getContractFactory("GnosisSafe");
        console.log("Deploying GnosisSafe...");
        const gnosisSafe = await GnosisSafe.deploy();
        console.log("GnosisSafe deployed to:", gnosisSafe.target);

        const MultiSend = await ethers.getContractFactory("MultiSend");
        console.log("Deploying MultiSend...");
        const multiSend = await MultiSend.deploy();
        console.log("MultiSend deployed to:", multiSend.target);

        const MultiSendCallOnly = await ethers.getContractFactory("MultiSendCallOnly");
        console.log("Deploying MultiSendCallOnly...");
        const multiSendCallOnly = await MultiSendCallOnly.deploy();
        console.log("MultiSendCallOnly deployed to:", multiSendCallOnly.target);

        const FallbackManager = await ethers.getContractFactory("FallbackManager");
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

        const SimulateTxAccessor = await ethers.getContractFactory("SimulateTxAccessor");
        console.log("Deploying SimulateTxAccessor...");
        const simulateTxAccessor = await SimulateTxAccessor.deploy();
        console.log("SimulateTxAccessor deployed to:", simulateTxAccessor.target);

        const chainId = await ethAdapter.getChainId()

        // await 1 block confirmations, else it gets finicky
        await singleton.deploymentTransaction().wait(1);
        await gnosisSafeProxyFactory.deploymentTransaction().wait(1);
        await multiSend.deploymentTransaction().wait(1);
        await multiSendCallOnly.deploymentTransaction().wait(1);
        await fallbackManager.deploymentTransaction().wait(1);
        await signMessageLib.deploymentTransaction().wait(1);
        await createCall.deploymentTransaction().wait(1);
        await simulateTxAccessor.deploymentTransaction().wait(1);

        const contractNetworks  = {
            [chainId]: {
                safeSingletonAddress:  gnosisSafe.target,
                safeProxyFactoryAddress:  gnosisSafeProxyFactory.target,
                multiSendAddress: multiSend.target,
                multiSendCallOnlyAddress: multiSendCallOnly.target,
                fallbackHandlerAddress: fallbackManager.target,
                signMessageLibAddress: signMessageLib.target,
                createCallAddress: createCall.target,
                simulateTxAccessorAddress: simulateTxAccessor.target,

                // safeProxy:  gnosisSafeProxy.target,
            }

        }

        console.log("Gnosis Safe contracts", contractNetworks);

        const safeFactory = await SafeFactory.create({ ethAdapter, contractNetworks, isL1SafeSingleton: true });

        // const safeSdk = await Safe.create({ ethAdapter, safeAddress, contractNetworks })
        
        // deploy new Safe
        const safeAccountConfig = {
            owners: [
                await signer0.getAddress(),
                // await signer1.getAddress(),
                // await signer2.getAddress()
            ],
            threshold: 1,
        }

        console.log("GnosisSafe config:", safeAccountConfig);

        const safe = await safeFactory.deploySafe({ safeAccountConfig });

        const safeAddress = await safe.getAddress();

        console.log("GnosisSafe:", safeAddress);

        // const safeSdk = await Safe.create({ ethAdapter, safeAddress: gnosisSafe.target, contractNetworks })

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
