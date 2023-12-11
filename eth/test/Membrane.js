const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const { loadFixture } = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const { execSync: exec } = require('child_process');

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

const TOKEN_AMOUNT = 1000;
const ALEPH_ACCOUNT = getRandomAlephAccount(3);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);
const COMMISSION_PER_DIX_MILLE = 30;
const MINIMUM_TRANSFER_AMOUNT_USD = 50;
const DIX_MILLE = 10000;

describe("Membrane", function () {
    describe("Constructor", function () {
        it("Reverts if threshold is 0", async () => {
            const signers = await ethers.getSigners();
            const accounts = signers.map(s => s.address)

            const Membrane = await ethers.getContractFactory("Membrane");
            await expect(upgrades.deployProxy(Membrane,
                                              [
                                                  [accounts[0]],
                                                  0,
                                                  COMMISSION_PER_DIX_MILLE,
                                                  MINIMUM_TRANSFER_AMOUNT_USD,
                                                  accounts [0],
                                              ],
                                              {
                                                  initializer: "initialize",
                                                  kind: 'uups'
                                              })).to.be.revertedWith("Signature threshold must be greater than 0");
        });
        it("Reverts if threshold is greater than number of guardians", async () => {
            const signers = await ethers.getSigners();
            const accounts = signers.map(s => s.address)

            const Membrane = await ethers.getContractFactory("Membrane");
            await expect(upgrades.deployProxy(Membrane,
                                              [
                                                  [accounts[0]],
                                                  2,
                                                  COMMISSION_PER_DIX_MILLE,
                                                  MINIMUM_TRANSFER_AMOUNT_USD,
                                                  accounts [0],
                                              ],
                                              {
                                                  initializer: "initialize",
                                                  kind: 'uups'
                                              })).to.be.revertedWith("Not enough guardians specified");

        });
    });

    async function deployEightGuardianMembraneFixture() {
        const signers = await ethers.getSigners();
        const accounts = signers.map(s => s.address)

        const Membrane = await ethers.getContractFactory("Membrane");
        const membrane = await upgrades.deployProxy(Membrane,
                                                    [
                                                        accounts.slice(1, 9),
                                                        5,
                                                        COMMISSION_PER_DIX_MILLE,
                                                        MINIMUM_TRANSFER_AMOUNT_USD,
                                                        accounts [0],
                                                    ],
                                                    {
                                                        initializer: "initialize",
                                                        kind: 'uups'
                                                    });
        const membraneAddress = await membrane.getAddress();

        const Token = await ethers.getContractFactory("Token");
        const token = await Token.deploy("10000000000000000000000000", "Shittoken", "SHIT");
        const tokenAddressBytes32 = addressToBytes32(await token.getAddress());

        const usdt = await Token.deploy("12000000000000000000000000", "Tether", "USDT");
        const usdtAddressBytes32 = addressToBytes32(await usdt.getAddress());

        await membrane.setUSDT(usdtAddressBytes32);
        return { membrane, token, tokenAddressBytes32, usdtAddressBytes32, membraneAddress };
    }

    describe("sendRequest", function () {
        it("Reverts if the USD value of the transfer amount is below the minimum", async () => {
            const { membrane, tokenAddressBytes32, usdtAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);

            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);

            amountToSend = await membrane.queryPrice(MINIMUM_TRANSFER_AMOUNT_USD - 1,
                                                     usdtAddressBytes32, // of
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

        it("Reverts if token transfer is not approved", async () => {
            const { membrane, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);

            await membrane.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
            await expect(
                membrane.sendRequest(
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ALEPH_ACCOUNT
                )
            ).to.be.reverted;
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
                0,
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
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
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
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
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
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
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
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
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
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
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

    describe("payoutRewards", function () {
        it("account can request a payout", async () => {
            const { membrane, token, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), TOKEN_AMOUNT * 2);

            for (let i = 1; i < 6 ; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                );
            }

            currentCommitteeId = await membrane.committeeId ();
            totalRewards = await membrane.getCollectedCommitteeRewards (currentCommitteeId, tokenAddressBytes32);

            await expect(
                currentCommitteeId
            ).to.be.equal(0);

            signerBalanceBefore = await token.balanceOf(accounts[1].address);

            await membrane.payoutRewards (currentCommitteeId,
                                         accounts[1].address,
                                         tokenAddressBytes32);

            signerBalanceAfter = await token.balanceOf(accounts[1].address);
            await expect(signerBalanceAfter).to.be.equal(signerBalanceBefore + (totalRewards / BigInt (8)));
        });

        it("past committee member can still request a payout", async () => {
            const { membrane, token, tokenAddressBytes32 } = await loadFixture(deployEightGuardianMembraneFixture);
            const accounts = await ethers.getSigners();
            const ethAddress = addressToBytes32(accounts[10].address);
            const requestHash = ethers.solidityPackedKeccak256(
                ["bytes32", "uint256", "bytes32", "uint256"],
                [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0]
            );

            // Provide funds for Membrane
            await token.transfer(await membrane.getAddress(), TOKEN_AMOUNT * 2);

            for (let i = 1; i < 6 ; i++) {
                await membrane.connect(accounts[i]).receiveRequest(
                    requestHash,
                    tokenAddressBytes32,
                    TOKEN_AMOUNT,
                    ethAddress,
                    0,
                );
            }

            previousCommitteeId = await membrane.committeeId ();

            await expect(
                previousCommitteeId
            ).to.be.equal(0);

            let committee = accounts.slice(2, 9).map((x) => x.address);
            let threshold = 4;

            await membrane.setCommittee (committee, threshold);

            await expect(
                await membrane.committeeId ()
            ).to.be.equal(1);

            totalRewards = await membrane.getCollectedCommitteeRewards (previousCommitteeId, tokenAddressBytes32);
            signerBalanceBefore = await token.balanceOf(accounts[1].address);

            await membrane.payoutRewards (previousCommitteeId,
                                         accounts[1].address,
                                         tokenAddressBytes32);

            signerBalanceAfter = await token.balanceOf(accounts[1].address);
            await expect(signerBalanceAfter).to.be.equal(signerBalanceBefore + (totalRewards / BigInt (8)));
        });
    });

    describe("Upgrade", function () {
        it("Membrane contract can be upgraded", async () => {
            exec('cp ./contracts/Membrane.sol ./contracts/MembraneV2.sol',
                 (error) => {
                     if (error !== null) {
                         console.log('exec error: ' + error);
                     }
                     exec('sed -i "17 a \ \ \ \ uint256 public test;" ./contracts/MembraneV2.sol',
                          async (error, stdout, stderr) => {
                              if (error !== null) {
                                  console.log('exec error: ' + error);
                              }

                              const { membrane, membraneAddress } = await loadFixture(deployEightGuardianMembraneFixture);

                              const accounts = await ethers.getSigners();
                              let committee = accounts.slice(2, 9).map((x) => x.address);
                              let threshold = 4;
                              await membrane.setCommittee (committee, threshold);

                              const MembraneV2 = await ethers.getContractFactory("MembraneV2");
                              const membraneV2 = await upgrades.upgradeProxy(membraneAddress, MembraneV2);

                              const address = await membraneV2.getAddress();
                              // address is preserved
                              await expect(address).to.be.equal(membraneAddress);

                              // state is preserved
                              await expect((membrane.isInCommittee (committee[0])));

                              // no state overwrite
                              await expect((membrane.test ())).to.be.equal(0);

                          });
                 });

            // clean up
            exec('rm ./contracts/MembraneV2.sol',
                 (error, stdout, stderr) => {
                     if (error !== null) {
                         console.log('exec error: ' + error);
                     }
                 });
        });
    });

});
