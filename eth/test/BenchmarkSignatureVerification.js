// ./test/ContractFactory.js
const BenchmarkSignatureVerification = artifacts.require("BenchmarkSignatureVerification");
const SimpleToken = artifacts.require("SimpleToken");

contract("BenchmarkSignatureVerification", accounts => {
  it(" deploy + estimate gas cost and successfully call bridgeTransfer and bridgeReceive.", async () => {

    await web3.eth.accounts.wallet.create(10)
    
    let addresses = [...Array(10).keys()].map(x => web3.eth.accounts.wallet[x].address);
    let keys = [...Array(10).keys()].map(x => web3.eth.accounts.wallet[x].privateKey);

    let guardianAddresses = addresses.slice(2, 10);
    let guardianKeys = keys.slice(2, 10);
    let nonce = 1;
    let external_nonce = 1;
    let thereshold = 5;

    const simpleTokenInstance = await SimpleToken.new({from: accounts[0]});
    const tokenAddress = simpleTokenInstance.address;
    const verificationBenchmarkInstance = await BenchmarkSignatureVerification.new(
        [tokenAddress], 
        guardianAddresses, 
        nonce,
        external_nonce,
        thereshold
    );

    // Gas estimate for bridgeTransfer
    let azAccount = "5DRbk3BimLzBzQtzUYUP35c57iwXtpqyjbnfY6AX48vbGMQC";

    // Allow the contract to spend 1000 tokens
    await simpleTokenInstance.approve(verificationBenchmarkInstance.address, 1000, {from: accounts[0]});

    const gasEstimateTransfer = await verificationBenchmarkInstance
        .bridgeTransfer
        .estimateGas(
            tokenAddress,
            azAccount,
            1000,
            {from: accounts[0]}
        );

    console.log("Gas estimate for bridgeTransfer: ", gasEstimateTransfer);

    const bridgeTransferTx = await verificationBenchmarkInstance.bridgeTransfer(
        tokenAddress,
        azAccount,
        1000,
        {gas: gasEstimateTransfer, from: accounts[0]}
    );
    assert(bridgeTransferTx.receipt.status == true, "bridgeTransfer failed");

    // Gas estimate for bridgeReceive
    let hashToSign = web3.utils.soliditySha3(nonce, tokenAddress, accounts[0], 1000); 
    let signatures = [...Array(thereshold).keys()].map(x => web3.eth.accounts.sign(hashToSign, guardianKeys[x]).signature);

    const gasEstimateReceive = await verificationBenchmarkInstance
        .bridgeReceive
        .estimateGas(
            tokenAddress,
            accounts[0],
            1000,
            signatures,
            {from: accounts[0]}
        );
    console.log("Gas estimate for bridgeReceive: ", gasEstimateReceive);

    const bridgeReceiveTx = await verificationBenchmarkInstance.bridgeReceive(
        tokenAddress,
        accounts[0],
        1000,
        signatures,
        {gas: gasEstimateReceive, from: accounts[0]}
    );
    assert(bridgeReceiveTx.receipt.status == true, "bridgeReceive failed");
  });
});