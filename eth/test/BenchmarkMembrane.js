const Membrane = artifacts.require("Membrane");
const TestToken = artifacts.require("TestToken");

// Import utils
const addressToBytes32 = require("./TestUtils").addressToBytes32;

contract("Membrane", accounts => {
    it(" deploy + estimate gas cost and successfully call sendRequest and receiveRequest.", async () => {
        // Transfer some eth to other accounts
        for (let i = 1; i < 10; i++) {
            await web3.eth.sendTransaction({
                from: accounts[0],
                to: accounts[i],
                value: web3.utils.toWei("1", "ether"),
            });
        }

        let guardianAddresses = accounts.slice(1, 9);
        let threshold = 5;

        const testTokenInstance = await TestToken.new({ from: accounts[0] });
        const tokenAddress = testTokenInstance.address;

        const membraneInstance = await Membrane.new(
            guardianAddresses,
            threshold,
            { from: accounts[0] }
        );

        // Easy way to get a "random" bytes32 value
        let azContract = web3.utils.soliditySha3(42);
        let tokenAddressBytes32 = addressToBytes32(tokenAddress);

        // Add pair of linked contracts
        await membraneInstance.addPair(
            tokenAddressBytes32,
            azContract,
            { from: accounts[0] }
        );

        // Gas estimate for sendRequest

        // bytes32 "address" of account on Aleph
        let azAccount = web3.utils.soliditySha3(0);

        // Allow Membrane to spend tokens
        await testTokenInstance.approve(membraneInstance.address, 1000, { from: accounts[0] });

        const gasEstimateSend = await membraneInstance
            .sendRequest
            .estimateGas(
                tokenAddressBytes32,
                1000,
                azAccount,
                { from: accounts[0] }
            );

        console.log("Gas estimate for sendRequest: ", gasEstimateSend);

        const sendRequestTx = await membraneInstance.sendRequest(
            tokenAddressBytes32,
            1000,
            azAccount,
            { gas: gasEstimateSend, from: accounts[0] }
        );
        assert(sendRequestTx.receipt.status == true, "sendRequest failed");


        // Gas estimate for bridgeReceive
        let ethAccount = web3.utils.soliditySha3(1);
        let requestHash = web3.utils.soliditySha3(tokenAddressBytes32, 1000, ethAccount, 1);

        // Estimate gas for each signature
        let gasEstimates = [...Array(threshold).keys()];
        for (let i = 0; i < threshold; i++) {
            gasEstimates[i] = await membraneInstance
                .receiveRequest
                .estimateGas(
                    requestHash,
                    tokenAddressBytes32,
                    1000,
                    ethAccount,
                    1,
                    { from: guardianAddresses[i] }
                );

            const receiveRequestTx = await membraneInstance.receiveRequest(
                requestHash,
                tokenAddressBytes32,
                1000,
                ethAccount,
                1,
                { gas: gasEstimates[i], from: guardianAddresses[i] }
            );
            assert(receiveRequestTx.receipt.status == true, "receiveRequest failed");
        }

        console.log("Gas estimates for receiveRequest: ", gasEstimates);

        // Sum gas estimates
        let sum = gasEstimates.reduce((a, b) => a + b, 0);
        console.log("Sum of gas estimates for receiveRequest: ", sum);
    });
});