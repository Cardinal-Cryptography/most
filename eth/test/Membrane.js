const { expect } = require('chai');
const hre = require("hardhat");
const { loadFixture } = require("@nomicfoundation/hardhat-toolbox/network-helpers");

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

describe("Membrane", function () {
    describe("Constructor", function () {
        it("Reverts if threshold is 0", async () => {
            const accounts = await hre.ethers.getSigners();
            const Membrane = await hre.ethers.getContractFactory("Membrane");
            await expect(
                Membrane.deploy([accounts[0].address], 0, { from: accounts[0] })
            ).to.be.revertedWith("Signature threshold must be greater than 0");
        });
        it("Reverts if threshold is greater than number of guardians", async () => {
            const accounts = await hre.ethers.getSigners();
            const Membrane = await hre.ethers.getContractFactory("Membrane");
            await expect(
                Membrane.deploy([accounts[0].address], 2, { from: accounts[0] })
            ).to.be.revertedWith("Not enough guardians specified");
        });
    });

    async function deployEightGuardianMembraneFixture() {
        const accounts = await hre.ethers.getSigners();
        const guardianKeys = accounts.slice(1, 9);
        const guardianAddresses = guardianKeys.map((x) => x.address);
        const threshold = 5;

        const Membrane = await hre.ethers.getContractFactory("Membrane");
        const membrane = await Membrane.deploy(
            guardianAddresses,
            threshold,
        );

        const TestToken = await hre.ethers.getContractFactory("TestToken");
        const token = await TestToken.deploy();

        return { membrane, token };
    }

    describe("sendRequest", function () {
        it("Reverts if token is not whitelisted", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const alephAccountBytes = getRandomAlephAccount(3);
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const membraneAddress = await membrane.getAddress();
            const amount = 1000;

            await token.approve(membraneAddress, amount);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    amount, 
                    alephAccountBytes
                )
            ).to.be.revertedWith("Unsupported pair");
        });
        it("Reverts if token is not approved", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const alephAccountBytes = getRandomAlephAccount(3);
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;

            await membrane.addPair(tokenAddressBytes32, getRandomAlephAccount(5));
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32, 
                    amount, 
                    alephAccountBytes
                )
            ).to.be.revertedWith("ERC20: insufficient allowance");
        });
        it("Transfers tokens to Membrane", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const alephAccountBytes = getRandomAlephAccount(3);
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const membraneAddress = await membrane.getAddress();
            const amount = 1000;

            await token.approve(membraneAddress, amount);
            await membrane.addPair(tokenAddressBytes32, getRandomAlephAccount(5));
            await membrane.sendRequest(
                tokenAddressBytes32, 
                amount, 
                alephAccountBytes
            );

            expect(await token.balanceOf(membraneAddress)).to.equal(amount);
        });
        it("Emits correct event", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const alephAccountBytes = getRandomAlephAccount(3);
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const membraneAddress = await membrane.getAddress();
            const amount = 1000;
            const wrappedTokenAddress = getRandomAlephAccount(5);

            await token.approve(membraneAddress, amount);
            await membrane.addPair(tokenAddressBytes32, wrappedTokenAddress);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    amount,
                    alephAccountBytes
                )
            ).to.emit(membrane, "CrosschainTransferRequest").withArgs(
                wrappedTokenAddress,
                amount,
                alephAccountBytes,
                0,
            );
        });
    });

    describe("receiveRequest", function () {
        it("Reverts if caller is not a guardian", async () => { 
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, amount, ethAddress, 0]
            );

            await expect(
                membrane.connect(accounts[0]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    addressToBytes32(accounts[10].address),
                    0,
                )
            ).to.be.revertedWith("Can only be called by a guardian");
        });
        it("Reverts if request has already been signed by a guardian", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, amount, ethAddress, 0]
            );

            await membrane.connect(accounts[1]).receiveRequest(
                requestHash,
                tokenAddressBytes32,
                amount,
                addressToBytes32(accounts[10].address),
                0,
            );
            await expect(
                membrane.connect(accounts[1]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("This guardian has already signed this request");
        });
        it("Reverts if request has already been executed", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, amount, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), amount * 2);

            for (let i = 1; i < 6; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    addressToBytes32(accounts[10].address),
                    0,
                );
            }

            await expect(
                membrane.connect(accounts[6]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("This request has already been processed");
        });
        it("Unlocks tokens for the user", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, amount, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), amount * 2);

            for (let i = 1; i < 6; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    addressToBytes32(accounts[10].address),
                    0,
                );
            }

            expect(await token.balanceOf(accounts[10].address)).to.equal(amount);
        });
        it("Reverts on non-matching hash", async () => {
            const { membrane, token } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
            const amount = 1000;
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, amount, ethAddress, 1]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), amount * 2);

            await expect(
                membrane.connect(accounts[1]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    amount,
                    addressToBytes32(accounts[10].address),
                    0,
                )
            ).to.be.revertedWith("Hash does not match the data");
        });
    });
});
