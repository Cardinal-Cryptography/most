const { expect } = require("chai");
const hre = require("hardhat");
const { loadFixture } = require("@nomicfoundation/hardhat-toolbox/network-helpers");

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

const TOKEN_AMOUNT = 1000;
const ALEPH_ACCOUNT = getRandomAlephAccount(3);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);
const USDT = "0x1000000000000000000000000000000000000000000000000000000000000000";
const COMMISSION_PER_DIX_MILLE = 30;
const MINIMUM_TRANSFER_AMOUNT_USD = 50;
const DIX_MILLE = 10000;

describe("Membrane", function () {
    describe("Constructor", function () {
        it("Reverts if threshold is 0", async () => {
            const accounts = await hre.ethers.getSigners();
            const Membrane = await hre.ethers.getContractFactory("Membrane");
            await expect(
                Membrane.deploy([accounts[0].address],
                                0,
                                COMMISSION_PER_DIX_MILLE,
                                MINIMUM_TRANSFER_AMOUNT_USD,
                                { from: accounts[0] })
            ).to.be.revertedWith("Signature threshold must be greater than 0");
        });
        it("Reverts if threshold is greater than number of guardians", async () => {
            const accounts = await hre.ethers.getSigners();
            const Membrane = await hre.ethers.getContractFactory("Membrane");
            await expect(
                Membrane.deploy([accounts[0].address],
                                2,
                                COMMISSION_PER_DIX_MILLE,
                                MINIMUM_TRANSFER_AMOUNT_USD,
                                { from: accounts[0] })
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
            COMMISSION_PER_DIX_MILLE,
            MINIMUM_TRANSFER_AMOUNT_USD
        );
        const membraneAddress = await membrane.getAddress();

        const TestToken = await hre.ethers.getContractFactory("TestToken");
        const token = await TestToken.deploy();
        const tokenAddressBytes32 = addressToBytes32(await token.getAddress());

        return { membrane, token, tokenAddressBytes32, membraneAddress };
    }

    describe("sendRequest", function () {

        // TODO
        it("Reverts if the USD value of the transfer amount is below the minimum", async () => {
            const { membrane, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);

            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);

            amountToSend = await membrane.queryPrice(MINIMUM_TRANSFER_AMOUNT_USD - 1,
                                                     USDT, // of
                                                     tokenAddressBytes32 // in
                                                    );

            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    amountToSend,
                    ALEPH_ACCOUNT
                )
            ).to.be.revertedWith("AmountBelowMinimum");
        });

        it("Reverts if token is not whitelisted", async () => {
            const { membrane, token, tokenAddressBytes32, membraneAddress } = await loadFixture(deployEightGuardianMembraneFixture);

            await token.approve(membraneAddress, TOKEN_AMOUNT);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ALEPH_ACCOUNT
                )
            ).to.be.revertedWith("Unsupported pair");
        });

        it("Reverts if token is not approved", async () => {
            const { membrane, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);

            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ALEPH_ACCOUNT
                )
            ).to.be.revertedWith("ERC20: insufficient allowance");
        });

        it("Transfers tokens to Membrane", async () => {
            const { membrane, token, tokenAddressBytes32, membraneAddress } = await loadFixture(deployEightGuardianMembraneFixture);

            await token.approve(membraneAddress, TOKEN_AMOUNT);
            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
            await membrane.sendRequest(
                tokenAddressBytes32,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT
            );

            expect(await token.balanceOf(membraneAddress)).to.equal(TOKEN_AMOUNT);
        });

        it("Emits correct event", async () => {
            const { membrane, token, tokenAddressBytes32, membraneAddress } = await loadFixture(deployEightGuardianMembraneFixture);

            await token.approve(membraneAddress, TOKEN_AMOUNT);
            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ALEPH_ACCOUNT
                )
            ).to.emit(membrane, "CrosschainTransferRequest").withArgs(
                WRAPPED_TOKEN_ADDRESS,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT,
                0,
            );
        });
    });

    describe("receiveRequest", function () {
        it("Reverts if caller is not a guardian", async () => {
            const { membrane, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            await expect(
                membrane.connect(accounts[0]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("NotInCommittee");
        });

        it("Reverts if request has already been signed by a guardian", async () => {
            const { membrane, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            await membrane.connect(accounts[1]).receiveRequest(
                requestHash,
                tokenAddressBytes32,
                TOKEN_AMOUNT,
                ethAddress,
                0,
            );
            await expect(
                membrane.connect(accounts[1]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("This guardian has already signed this request");
        });

        it("Reverts if request has already been executed", async () => {
            const { membrane, token, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), TOKEN_AMOUNT * 2);

            for (let i = 1; i < 6; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                );
            }

            await expect(
                membrane.connect(accounts[6]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("This request has already been processed");
        });

        it("Unlocks tokens for the user", async () => {
            const { membrane, token, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), TOKEN_AMOUNT * 2);

            for (let i = 1; i < 6; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                );
            }

            expect(await token.balanceOf(accounts[10].address)).to.equal(TOKEN_AMOUNT * (DIX_MILLE - COMMISSION_PER_DIX_MILLE) / DIX_MILLE);
        });

        it("Reverts on non-matching hash", async () => {
            const { membrane, token, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await hre.ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = hre.ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 1]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), TOKEN_AMOUNT * 2);

            await expect(
                membrane.connect(accounts[1]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                )
            ).to.be.revertedWith("Hash does not match the data");
        });
    });
});
