// ./test/ContractFactory.js
const BenchmarkSignatureAggregation = artifacts.require("BenchmarkSignatureAggregation");

contract("BenchmarkSignatureAggregation", accounts => {
  it(" deploy + estimate gas cost and successfully call bridgeTransfer and bridgeReceive.", async () => {

    await web3.eth.accounts.wallet.create(10)
    
    let addresses = [...Array(10).keys()].map(x => web3.eth.accounts.wallet[x].address);
    let keys = [...Array(10).keys()].map(x => web3.eth.accounts.wallet[x].privateKey);

    let tokenWhitelist = addresses.slice(0, 2);
    let guardianAddresses = addresses.slice(2, 10);
    let guardianKeys = keys.slice(2, 10);
    let nonce = 1;
    let external_nonce = 1;
    let thereshold = 5;

    const aggregationBenchmarkInstance = await BenchmarkSignatureAggregation.new(
        tokenWhitelist, 
        guardianAddresses, 
        nonce,
        external_nonce,
        thereshold,
        {from: accounts[0]}
    );

    // Gas estimate for bridgeTransfer
    let azAccount = "5DRbk3BimLzBzQtzUYUP35c57iwXtpqyjbnfY6AX48vbGMQC";

    const gasEstimateTransfer = await aggregationBenchmarkInstance
        .bridgeTransfer
        .estimateGas(
            tokenWhitelist[0],
            azAccount,
            1000,
            {from: accounts[0]}
        );

    console.log("Gas estimate for bridgeTransfer: ", gasEstimateTransfer);

    const bridgeTransferTx = await aggregationBenchmarkInstance.bridgeTransfer(
        tokenWhitelist[0],
        azAccount,
        1000,
        {gas: gasEstimateTransfer, from: accounts[0]}
    );
    assert(bridgeTransferTx.receipt.status == true, "bridgeTransfer failed");

    // Gas estimate for bridgeReceive

    // Transfer some ether to the guardian addresses
    for (let i = 0; i < thereshold; i++) {
        await web3.eth.sendTransaction({
            from: accounts[0],
            to: guardianAddresses[i],
            value: web3.utils.toWei('0.05', 'ether')
        });

        // Important to add account and unlock it
        await web3.eth.personal.importRawKey(guardianKeys[i].slice(2), process.env.ACCOUNT_PASSWORD);
        await web3.eth.personal.unlockAccount(guardianAddresses[i], process.env.ACCOUNT_PASSWORD, 10000);
    }

    // Estimate gas for each signature
    let gasEstimates = [...Array(thereshold).keys()];
    for (let i = 0; i < thereshold; i++) {
        gasEstimates[i] = await aggregationBenchmarkInstance
            .bridgeReceive
            .estimateGas(
                tokenWhitelist[0],
                accounts[0],
                1000,
                nonce,
                {from: guardianAddresses[i]}
            );
        
        const bridgeReceiveTx = await aggregationBenchmarkInstance.bridgeReceive(
            tokenWhitelist[0],
            accounts[0],
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

    // Remove temporary accounts
    for (let i = 0; i < thereshold; i++) {
        await web3.eth.accounts.wallet.remove(guardianAddresses[i]);
    }
  });
});