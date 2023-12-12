const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const { execSync: exec } = require("child_process");

// Import utils
const { addressToBytes32, getRandomAlephAccount } = require("./TestUtils");

const TOKEN_AMOUNT = 1000;
const ALEPH_ACCOUNT = getRandomAlephAccount(3);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);
const COMMISSION_PER_DIX_MILLE = 30;
const MINIMUM_TRANSFER_AMOUNT_USD = 50;
const DIX_MILLE = 10000;

describe("Most", function () {
  describe("Constructor", function () {
    it("Reverts if threshold is 0", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);

      const Most = await ethers.getContractFactory("Most");
      await expect(
        upgrades.deployProxy(
          Most,
          [
            [accounts[0]],
            0,
            COMMISSION_PER_DIX_MILLE,
            MINIMUM_TRANSFER_AMOUNT_USD,
            accounts[0],
          ],
          {
            initializer: "initialize",
            kind: "uups",
          },
        ),
      ).to.be.revertedWith("Signature threshold must be greater than 0");
    });
    it("Reverts if threshold is greater than number of guardians", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);

      const Most = await ethers.getContractFactory("Most");
      await expect(
        upgrades.deployProxy(
          Most,
          [
            [accounts[0]],
            2,
            COMMISSION_PER_DIX_MILLE,
            MINIMUM_TRANSFER_AMOUNT_USD,
            accounts[0],
          ],
          {
            initializer: "initialize",
            kind: "uups",
          },
        ),
      ).to.be.revertedWith("Not enough guardians specified");
    });
  });

  async function deployEightGuardianMostFixture() {
    const signers = await ethers.getSigners();
    const accounts = signers.map((s) => s.address);

    const Most = await ethers.getContractFactory("Most");
    const most = await upgrades.deployProxy(
      Most,
      [
        accounts.slice(1, 9),
        5,
        COMMISSION_PER_DIX_MILLE,
        MINIMUM_TRANSFER_AMOUNT_USD,
        accounts[0],
      ],
      {
        initializer: "initialize",
        kind: "uups",
      },
    );
    const mostAddress = await most.getAddress();

    const Token = await ethers.getContractFactory("Token");
    const token = await Token.deploy(
      "10000000000000000000000000",
      "Shittoken",
      "SHIT",
    );
    const tokenAddressBytes32 = addressToBytes32(await token.getAddress());

    const usdt = await Token.deploy(
      "12000000000000000000000000",
      "Tether",
      "USDT",
    );
    const usdtAddressBytes32 = addressToBytes32(await usdt.getAddress());

    await most.setUSDT(usdtAddressBytes32);
    return {
      most,
      token,
      tokenAddressBytes32,
      usdtAddressBytes32,
      mostAddress,
    };
  }

  describe("sendRequest", function () {
    it("Reverts if the USD value of the transfer amount is below the minimum", async () => {
      const { most, tokenAddressBytes32, usdtAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);

      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);

      amountToSend = await most.queryPrice(
        MINIMUM_TRANSFER_AMOUNT_USD - 1,
        usdtAddressBytes32, // of
        tokenAddressBytes32, // in
      );

      await expect(
        most.sendRequest(tokenAddressBytes32, amountToSend, ALEPH_ACCOUNT),
      ).to.be.revertedWith("AmountBelowMinimum");
    });

    it("Reverts if token is not whitelisted", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWith("Unsupported pair");
    });

    it("Reverts if token transfer is not approved", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );

      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.reverted;
    });

    it("Transfers tokens to Most", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
      await most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT);

      expect(await token.balanceOf(mostAddress)).to.equal(TOKEN_AMOUNT);
    });

    it("Emits correct event", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      )
        .to.emit(most, "CrosschainTransferRequest")
        .withArgs(0, WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT, 0);
    });
  });

  describe("receiveRequest", function () {
    it("Reverts if caller is not a guardian", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      await expect(
        most
          .connect(accounts[0])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWith("NotInCommittee");
    });

    it("Reverts if request has already been signed by a guardian", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      await most
        .connect(accounts[1])
        .receiveRequest(
          requestHash,
          tokenAddressBytes32,
          TOKEN_AMOUNT,
          ethAddress,
          0,
        );
      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWith("This guardian has already signed this request");
    });

    it("Ignores already executed requests", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      await expect(
        most
          .connect(accounts[6])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      )
        .to.emit(most, "ProcessedRequestSigned")
        .withArgs(requestHash, accounts[6].address);
    });

    it("Unlocks tokens for the user", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      expect(await token.balanceOf(accounts[10].address)).to.equal(
        (TOKEN_AMOUNT * (DIX_MILLE - COMMISSION_PER_DIX_MILLE)) / DIX_MILLE,
      );
    });

    it("Reverts on non-matching hash", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 1],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWith("Hash does not match the data");
    });
  });

  describe("payoutRewards", function () {
    it("account can request a payout", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      currentCommitteeId = await most.committeeId();
      totalRewards = await most.getCollectedCommitteeRewards(
        currentCommitteeId,
        tokenAddressBytes32,
      );

      await expect(currentCommitteeId).to.be.equal(0);

      signerBalanceBefore = await token.balanceOf(accounts[1].address);

      await most.payoutRewards(
        currentCommitteeId,
        accounts[1].address,
        tokenAddressBytes32,
      );

      signerBalanceAfter = await token.balanceOf(accounts[1].address);
      await expect(signerBalanceAfter).to.be.equal(
        signerBalanceBefore + totalRewards / BigInt(8),
      );
    });

    it("past committee member can still request a payout", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["bytes32", "uint256", "bytes32", "uint256"],
        [tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      previousCommitteeId = await most.committeeId();

      await expect(previousCommitteeId).to.be.equal(0);

      let committee = accounts.slice(2, 9).map((x) => x.address);
      let threshold = 4;

      await most.setCommittee(committee, threshold);

      await expect(await most.committeeId()).to.be.equal(1);

      totalRewards = await most.getCollectedCommitteeRewards(
        previousCommitteeId,
        tokenAddressBytes32,
      );
      signerBalanceBefore = await token.balanceOf(accounts[1].address);

      await most.payoutRewards(
        previousCommitteeId,
        accounts[1].address,
        tokenAddressBytes32,
      );

      signerBalanceAfter = await token.balanceOf(accounts[1].address);
      await expect(signerBalanceAfter).to.be.equal(
        signerBalanceBefore + totalRewards / BigInt(8),
      );
    });
  });

  describe("Upgrade", function () {
    it("Most contract can be upgraded", async () => {
      exec("cp ./contracts/Most.sol ./contracts/MostV2.sol", (error) => {
        if (error !== null) {
          console.log("exec error: " + error);
        }
        exec(
          'sed -i "17 a     uint256 public test;" ./contracts/MostV2.sol',
          async (error, stdout, stderr) => {
            if (error !== null) {
              console.log("exec error: " + error);
            }

            const { most, mostAddress } = await loadFixture(
              deployEightGuardianMostFixture,
            );

            const accounts = await ethers.getSigners();
            let committee = accounts.slice(2, 9).map((x) => x.address);
            let threshold = 4;
            await most.setCommittee(committee, threshold);

            const MostV2 = await ethers.getContractFactory("MostV2");
            const mostV2 = await upgrades.upgradeProxy(mostAddress, MostV2);

            const address = await mostV2.getAddress();
            // address is preserved
            expect(address).to.be.equal(mostAddress);

            // state is preserved
            expect(most.isInCommittee(committee[0]));

            // no state overwrite
            expect(most.test()).to.be.equal(0);
          },
        );
      });

      // clean up
      exec("rm ./contracts/MostV2.sol", (error, stdout, stderr) => {
        if (error !== null) {
          console.log("exec error: " + error);
        }
      });
    });
  });
});
