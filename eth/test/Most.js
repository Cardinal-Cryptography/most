const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
  setBalance,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const { execSync: exec } = require("child_process");

// Import utils
const {
  addressToBytes32,
  getRandomAlephAccount,
  ethToWei,
} = require("./TestUtils");

const TOKEN_AMOUNT = 1000;
const ALEPH_ACCOUNT = getRandomAlephAccount(3);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);

describe("Most", function () {
  describe("Constructor", function () {
    it("Reverts if threshold is 0", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);

      const WETH = await ethers.getContractFactory("WETH9");
      const weth = await WETH.deploy();

      const Most = await ethers.getContractFactory("Most");
      await expect(
        upgrades.deployProxy(
          Most,
          [[accounts[0]], 0, accounts[0], await weth.getAddress()],
          {
            initializer: "initialize",
            kind: "uups",
          },
        ),
      ).to.be.revertedWithCustomError(Most, "ZeroSignatureTreshold");
    });
    it("Reverts if threshold is greater than number of guardians", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);

      const WETH = await ethers.getContractFactory("WETH9");
      const weth = await WETH.deploy();

      const Most = await ethers.getContractFactory("Most");
      await expect(
        upgrades.deployProxy(
          Most,
          [[accounts[0]], 2, accounts[0], await weth.getAddress()],
          {
            initializer: "initialize",
            kind: "uups",
          },
        ),
      ).to.be.revertedWithCustomError(Most, "NotEnoughGuardians");
    });
    it("Reverts if duplicate guardians", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);

      const WETH = await ethers.getContractFactory("WETH9");
      const weth = await WETH.deploy();

      const Most = await ethers.getContractFactory("Most");
      await expect(
        upgrades.deployProxy(
          Most,
          [
            [accounts[0], accounts[1], accounts[2], accounts[0]],
            2,
            accounts[0],
            await weth.getAddress(),
          ],
          {
            initializer: "initialize",
            kind: "uups",
          },
        ),
      ).to.be.revertedWithCustomError(Most, "DuplicateCommitteeMember");
    });
  });

  async function deployEightGuardianMostFixture() {
    const signers = await ethers.getSigners();
    const accounts = signers.map((s) => s.address);

    const WETH = await ethers.getContractFactory("WETH9");
    const weth = await WETH.deploy();
    const wethAddress = await weth.getAddress();

    const Most = await ethers.getContractFactory("Most");
    const most = await upgrades.deployProxy(
      Most,
      [accounts.slice(1, 9), 5, accounts[0], wethAddress],
      {
        initializer: "initialize",
        kind: "uups",
      },
    );
    const mostAddress = await most.getAddress();
    await most.unpause();

    const Token = await ethers.getContractFactory("Token");
    const token = await Token.deploy(
      "10000000000000000000000000",
      "18",
      "TestToken",
      "TEST",
    );
    const tokenAddressBytes32 = addressToBytes32(await token.getAddress());

    return {
      most,
      token,
      weth,
      tokenAddressBytes32,
      mostAddress,
      wethAddress,
    };
  }

  describe("sendRequest", function () {
    it("Reverts if token is not whitelisted", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWithCustomError(most, "UnsupportedPair");
    });

    it("Reverts if token transfer is not approved", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );

      await most.pause();
      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS, false);
      await most.unpause();
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.reverted;
    });

    it("Transfers tokens to Most", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await most.pause();
      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS, false);
      await most.unpause();
      await most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT);

      expect(await token.balanceOf(mostAddress)).to.equal(TOKEN_AMOUNT);
    });

    it("Emits correct event", async () => {
      const { most, token, tokenAddressBytes32, mostAddress } =
        await loadFixture(deployEightGuardianMostFixture);

      await token.approve(mostAddress, TOKEN_AMOUNT);
      await most.pause();
      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS, false);
      await most.unpause();
      await expect(
        most.sendRequest(tokenAddressBytes32, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      )
        .to.emit(most, "CrosschainTransferRequest")
        .withArgs(0, WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT, 0);
    });
  });

  describe("sendRequestNative", function () {
    it("Reverts if token is not whitelisted", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);

      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, { value: TOKEN_AMOUNT }),
      ).to.be.revertedWithCustomError(most, "UnsupportedPair");
    });

    it("Transfers tokens to Most", async () => {
      const { most, mostAddress, wethAddress, weth } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      await most.pause();
      await most.addPair(
        addressToBytes32(wethAddress),
        WRAPPED_TOKEN_ADDRESS,
        false,
      );
      await most.unpause();
      await most.sendRequestNative(ALEPH_ACCOUNT, { value: TOKEN_AMOUNT });

      expect(await weth.balanceOf(mostAddress)).to.equal(TOKEN_AMOUNT);
    });

    it("Emits correct event", async () => {
      const { most, wethAddress } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      await most.pause();
      await most.addPair(
        addressToBytes32(wethAddress),
        WRAPPED_TOKEN_ADDRESS,
        false,
      );
      await most.unpause();
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, { value: TOKEN_AMOUNT }),
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
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      await expect(
        most
          .connect(accounts[0])
          .receiveRequest(
            requestHash,
            0,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "NotInCommittee");
    });

    it("Ignores consecutive signatures", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      await most
        .connect(accounts[1])
        .receiveRequest(
          requestHash,
          0,
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
            0,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      )
        .to.emit(most, "RequestAlreadySigned")
        .withArgs(requestHash, accounts[1].address);
    });

    it("Ignores already executed requests", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            0,
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
            0,
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
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            0,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      expect(await token.balanceOf(accounts[10].address)).to.equal(
        TOKEN_AMOUNT,
      );
    });

    it("Reverts on non-matching hash", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 1],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            requestHash,
            0,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "DataHashMismatch");
    });

    it("Committee rotation", async () => {
      const { most, token, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHashOld = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );
      const requestHashNew = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [1, tokenAddressBytes32, TOKEN_AMOUNT, ethAddress, 0],
      );

      // Provide funds for Most
      await token.transfer(await most.getAddress(), TOKEN_AMOUNT * 2);

      // Check committee size
      expect(await most.committeeSize(0)).to.be.equal(8);

      // Rotate committee
      await most.connect(accounts[0]).pause();
      await most.connect(accounts[0]).setCommittee(accounts.slice(3, 10), 5);
      await most.connect(accounts[0]).unpause();

      // Check new committee size
      expect(await most.committeeSize(1)).to.be.equal(7);

      await most
        .connect(accounts[2])
        .receiveRequest(
          requestHashOld,
          0,
          tokenAddressBytes32,
          TOKEN_AMOUNT,
          ethAddress,
          0,
        );

      await most
        .connect(accounts[9])
        .receiveRequest(
          requestHashNew,
          1,
          tokenAddressBytes32,
          TOKEN_AMOUNT,
          ethAddress,
          0,
        );

      await expect(
        most
          .connect(accounts[2])
          .receiveRequest(
            requestHashNew,
            1,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "NotInCommittee");

      await expect(
        most
          .connect(accounts[9])
          .receiveRequest(
            requestHashOld,
            0,
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "NotInCommittee");
    });
  });

  describe("receiveRequestNative", function () {
    it("Unlocks tokens for the user", async () => {
      const { most, weth, wethAddress, mostAddress } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const zeroAddress = "0x0000000000000000000000000000000000000000";
      const token_amount = ethToWei(TOKEN_AMOUNT);
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, addressToBytes32(zeroAddress), token_amount, ethAddress, 0],
      );

      const provider = await hre.ethers.provider;
      // Provide funds for Most
      await weth.deposit({ value: token_amount });
      expect(await weth.balanceOf(accounts[0].address)).to.equal(token_amount);
      await weth.transfer(mostAddress, token_amount);
      expect(await weth.balanceOf(mostAddress)).to.equal(token_amount);

      const balanceBefore = await provider.getBalance(accounts[10].address);
      const balanceBeforeMost = await provider.getBalance(mostAddress);

      for (const signer of accounts.slice(1, 6)) {
        await most
          .connect(signer)
          .receiveRequest(
            requestHash,
            0,
            addressToBytes32(zeroAddress),
            token_amount,
            ethAddress,
            0,
          );
      }
      const balanceAfter = await provider.getBalance(accounts[10].address);
      const balanceAfterMost = await provider.getBalance(mostAddress);

      expect(await weth.balanceOf(mostAddress)).to.equal(0);
      expect(await weth.balanceOf(accounts[10].address)).to.equal(0);
      expect(balanceAfterMost - balanceBeforeMost).to.equal(0);
      expect(balanceAfter - balanceBefore).to.equal(token_amount);
    });

    it("Unsuccessful transfer to a contract fails with event", async () => {
      const { most, weth, wethAddress, mostAddress, token } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const zeroAddress = "0x0000000000000000000000000000000000000000";
      const token_amount = ethToWei(TOKEN_AMOUNT);
      const accounts = await ethers.getSigners();
      // token contract doesn't accept ether so any native ether transfer to it will fail
      const ethAddress = token.target;
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [
          0,
          addressToBytes32(zeroAddress),
          token_amount,
          addressToBytes32(ethAddress),
          0,
        ],
      );

      const provider = await hre.ethers.provider;
      // Provide funds for Most
      await weth.deposit({ value: token_amount });
      expect(await weth.balanceOf(accounts[0].address)).to.equal(token_amount);
      await weth.transfer(mostAddress, token_amount);
      expect(await weth.balanceOf(mostAddress)).to.equal(token_amount);

      const balanceBefore = await provider.getBalance(ethAddress);
      const balanceBeforeMost = await provider.getBalance(mostAddress);

      for (const signer of accounts.slice(1, 5)) {
        await most
          .connect(signer)
          .receiveRequest(
            requestHash,
            0,
            addressToBytes32(zeroAddress),
            token_amount,
            addressToBytes32(ethAddress),
            0,
          );
      }

      const res = expect(
        most
          .connect(accounts[5])
          .receiveRequest(
            requestHash,
            0,
            addressToBytes32(zeroAddress),
            token_amount,
            addressToBytes32(ethAddress),
            0,
          ),
      );
      await res.to.emit(most, "EthTransferFailed").withArgs(requestHash);

      await res.to.emit(most, "RequestProcessed").withArgs(requestHash);

      const balanceAfter = await provider.getBalance(ethAddress);
      const balanceAfterMost = await provider.getBalance(mostAddress);

      expect(await weth.balanceOf(ethAddress)).to.equal(0);
      expect(await weth.balanceOf(mostAddress)).to.equal(0);

      // we expect that native ether is locked in the most contract
      expect(balanceAfterMost - balanceBeforeMost).to.equal(token_amount);
      expect(balanceAfter - balanceBefore).to.equal(0);
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

            const { most, mostAddress, token, tokenAddressBytes32 } =
              await loadFixture(deployEightGuardianMostFixture);

            await token.approve(mostAddress, TOKEN_AMOUNT);
            await most.pause();
            await most.addPair(
              tokenAddressBytes32,
              WRAPPED_TOKEN_ADDRESS,
              false,
            );
            await most.unpause();

            // sending request works before the upgrade
            await expect(
              most.sendRequest(
                tokenAddressBytes32,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT,
              ),
            )
              .to.emit(most, "CrosschainTransferRequest")
              .withArgs(
                0,
                WRAPPED_TOKEN_ADDRESS,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT,
                0,
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

            // sending request works after the upgrade
            await token.approve(mostAddress, TOKEN_AMOUNT);
            await expect(
              most.sendRequest(
                tokenAddressBytes32,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT,
              ),
            )
              .to.emit(most, "CrosschainTransferRequest")
              .withArgs(
                0,
                WRAPPED_TOKEN_ADDRESS,
                TOKEN_AMOUNT,
                ALEPH_ACCOUNT,
                1,
              );
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
