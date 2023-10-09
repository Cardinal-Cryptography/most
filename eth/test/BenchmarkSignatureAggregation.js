// ./test/ContractFactory.js
const BenchmarkSignatureAggregation = artifacts.require("BenchmarkSignatureAggregation");
const SimpleToken = artifacts.require("SimpleToken");

contract("BenchmarkSignatureAggregation", accounts => {
  it(" deploy + estimate gas cost and successfully call bridgeTransfer and bridgeReceive.", async () => {

    let guardianAddresses = accounts.slice(1, 9);
    let nonce = 1;
    let external_nonce = 1;
    let thereshold = 5;

    const simpleTokenInstance = await SimpleToken.new({from: accounts[0]});
    const tokenAddress = simpleTokenInstance.address;
    const aggregationBenchmarkInstance = await BenchmarkSignatureAggregation.new(
        [tokenAddress], 
        guardianAddresses, 
        nonce,
        external_nonce,
        thereshold,
        {from: accounts[0]}
    );

    // Gas estimate for bridgeTransfer
    let azAccount = "5DRbk3BimLzBzQtzUYUP35c57iwXtpqyjbnfY6AX48vbGMQC";

    // Allow the contract to spend 1000 tokens
    await simpleTokenInstance.approve(aggregationBenchmarkInstance.address, 1000, {from: accounts[0]});

    const gasEstimateTransfer = await aggregationBenchmarkInstance
        .bridgeTransfer
        .estimateGas(
            tokenAddress,
            azAccount,
            1000,
            {from: accounts[0]}
        );

    console.log("Gas estimate for bridgeTransfer: ", gasEstimateTransfer);

    const bridgeTransferTx = await aggregationBenchmarkInstance.bridgeTransfer(
        tokenAddress,
        azAccount,
        1000,
        {gas: gasEstimateTransfer, from: accounts[0]}
    );
    assert(bridgeTransferTx.receipt.status == true, "bridgeTransfer failed");

    // Gas estimate for bridgeReceive

    // Estimate gas for each signature
    let gasEstimates = [...Array(thereshold).keys()];
    for (let i = 0; i < thereshold; i++) {
        gasEstimates[i] = await aggregationBenchmarkInstance
            .bridgeReceive
            .estimateGas(
                tokenAddress,
                accounts[9],
                1000,
                nonce,
                {from: guardianAddresses[i]}
            );
        
        const bridgeReceiveTx = await aggregationBenchmarkInstance.bridgeReceive(
            tokenAddress,
            accounts[9],
            1000,
            nonce,
            {gas: gasEstimates[i], from: guardianAddresses[i]}
        );
        assert(bridgeReceiveTx.receipt.status == true, "bridgeReceive failed");
    }

    console.log("Gas estimates for bridgeReceive: ", gasEstimates);

    // Sum gas estimates
    let sum = gasEstimates.reduce((a, b) => a + b, 0);
    console.log("Sum of gas estimates for bridgeReceive: ", sum);
  });
});