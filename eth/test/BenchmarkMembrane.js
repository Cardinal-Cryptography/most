const Membrane = artifacts.require("Membrane");
const SimpleToken = artifacts.require("SimpleToken");

contract("Membrane", accounts => {
  it(" deploy + estimate gas cost and successfully call sendRequest and receiveRequest.", async () => {
    // Unlock accounts
    for (let i = 0; i < accounts.length; i++) {
        await web3.eth.personal.unlockAccount(accounts[i], process.env.ACCOUNT_PASSWORD, 86400);
    }

    let guardianAddresses = accounts.slice(1, 9);
    let thereshold = 5;

    const simpleTokenInstance = await SimpleToken.new({from: accounts[0]});
    const tokenAddress = simpleTokenInstance.address;

    const membraneBenchmarkInstance = await Membrane.new(
        guardianAddresses, 
        thereshold,
        {from: accounts[0]}
    );

    // Gas estimate for sendRequest
    let azAccount = "5DRbk3BimLzBzQtzUYUP35c57iwXtpqyjbnfY6AX48vbGMQC";

    // Allow membrane to spend tokens
    await simpleTokenInstance.approve(membraneBenchmarkInstance.address, 1000, {from: accounts[0]});

    const gasEstimateSend = await membraneBenchmarkInstance
        .sendRequest
        .estimateGas(
            tokenAddress,
            1000,
            azAccount,
            azAccount,
            {from: accounts[0]}
        );

    console.log("Gas estimate for sendRequest: ", gasEstimateSend);

    const sendRequestTx = await membraneBenchmarkInstance.sendRequest(
        tokenAddress,
        1000,
        azAccount,
        azAccount,
        {gas: gasEstimateSend, from: accounts[0]}
    );
    assert(sendRequestTx.receipt.status == true, "sendRequest failed");

    
    // Gas estimate for bridgeReceive
    let requestHash = web3.utils.soliditySha3(tokenAddress, 1000, accounts[9], 1);

    // Estimate gas for each signature
    let gasEstimates = [...Array(thereshold).keys()];
    for (let i = 0; i < thereshold; i++) {
        gasEstimates[i] = await membraneBenchmarkInstance
            .receiveRequest
            .estimateGas(
                requestHash,
                tokenAddress,
                1000,
                accounts[9],
                {from: guardianAddresses[i]}
            );
        
        const receiveRequestTx = await membraneBenchmarkInstance.receiveRequest(
            requestHash,
            tokenAddress,
            1000,
            accounts[9],
            {gas: gasEstimates[i], from: guardianAddresses[i]}
        );
        assert(receiveRequestTx.receipt.status == true, "receiveRequest failed");
    }

    console.log("Gas estimates for receiveRequest: ", gasEstimates);

    // Sum gas estimates
    let sum = gasEstimates.reduce((a, b) => a + b, 0);
    console.log("Sum of gas estimates for receiveRequest: ", sum);
  });
});