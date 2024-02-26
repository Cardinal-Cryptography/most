const { SafeFactory } = '@gnosis.pm/safe-core-sdk'

async function main() {

    switch (network.name) {
    case 'development':
        // TODO : deploy
        const GnosisSafeProxyFactory = await ethers.getContractFactory("GnosisSafeProxyFactory");
        console.log("Deploying GnosisSafeProxyFactory...");
        const gnosisSafeProxyFactory = await GnosisSafeProxyFactory.deploy();
        console.log("GnosisSafeProxyFactory deployed to:", gnosisSafeProxyFactory.target);      

        const GnosisSafe = await ethers.getContractFactory("GnosisSafe");
        console.log("Deploying GnosisSafe...");      
        const gnosisSafe = await GnosisSafe.deploy();
        console.log("GnosisSafe deployed to:", gnosisSafe.target);      

        const MultiSend = await ethers.getContractFactory("MultiSend");
        console.log("Deploying MultiSend...");      
        const multiSend = await MultiSend.deploy();
        console.log("MultiSend deployed to:", multiSend.target);      

        console.log("@@@@ MultiSend obj:", MultiSend);      

        process.exit(0);
        
        break;
        // TODO: for other networks augment hardhat config with the addresses of the already deployed gnosis contracts
    default:
        console.log(`Unknown network ${network.name}`);
        process.exit(-1);
    }

}


