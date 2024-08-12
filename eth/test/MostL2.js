const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const { execSync: exec } = require("child_process");

// Import utils
const {
  addressToBytes32,
  getRandomAlephAccount,
  getRandomAddress,
} = require("./TestUtils");

const TOKEN_AMOUNT = 10000000000;
const ALEPH_ACCOUNT = getRandomAlephAccount(3);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);

describe("MostL2", function () {
  describe("Constructor", function () {
    it("Reverts if threshold is 0", async () => {
      const signers = await ethers.getSigners();
      const accounts = signers.map((s) => s.address);
      const poolAddress = getRandomAddress();
      const bazeroAddress = getRandomAddress();

      const Most = await ethers.getContractFactory("MostL2");
      await expect(
        upgrades.deployProxy(
          Most,
          [[accounts[0]], 0, accounts[0], poolAddress, bazeroAddress],
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

      const poolAddress = getRandomAddress();
      const bazeroAddress = getRandomAddress();

      const Most = await ethers.getContractFactory("MostL2");
      await expect(
        upgrades.deployProxy(
          Most,
          [[accounts[0]], 2, accounts[0], poolAddress, bazeroAddress],
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

      const poolAddress = getRandomAddress();
      const bazeroAddress = getRandomAddress();

      const Most = await ethers.getContractFactory("MostL2");
      await expect(
        upgrades.deployProxy(
          Most,
          [
            [accounts[0], accounts[1], accounts[2], accounts[0]],
            2,
            accounts[0],
            poolAddress,
            bazeroAddress,
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
    const WrappedToken = await ethers.getContractFactory("WrappedToken");
    const bazero = await WrappedToken.deploy(
      "Bridge Azero",
      "bazero",
      12,
      accounts[0],
    );
    await bazero.waitForDeployment();

    const nativeToken = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

    const LP = await ethers.getContractFactory("StableSwapLP");
    const lp = await LP.deploy();
    await lp.waitForDeployment();

    const Pool = await ethers.getContractFactory("StableSwapTwoPool");
    const pool = await Pool.deploy();
    await pool.waitForDeployment();

    const setMinterTX = await lp.setMinter(pool.target);
    await setMinterTX.wait(1);

    const MostL2 = await ethers.getContractFactory("MostL2");
    const most = await upgrades.deployProxy(
      MostL2,
      [accounts.slice(1, 9), 5, accounts[0], pool.target, bazero.target],
      {
        initializer: "initialize",
        kind: "uups",
      },
    );
    await most.waitForDeployment();
    await bazero.mint(accounts[0], 1000000n * 10n ** 12n);
    await bazero.setMinterBurner(most.target);
    await bazero.approve(pool.target, 100n * 10n ** 12n);

    const initializeTx = await pool.initialize(
      [bazero.target, nativeToken],
      2000, // Todo set constants, fees etc
      0,
      0,
      accounts[0],
      lp.target,
    );
    await initializeTx.wait(1);

    await pool.add_liquidity([100n * 10n ** 12n, 100n * 10n ** 18n], 0, {
      value: ethers.parseEther("100.0"),
    });
    const bazeroAdrressBytes = ethers.zeroPadValue(
      ethers.getBytes(bazero.target),
      32,
    );

    await most.unpause();

    return {
      most,
      bazero,
      bazeroAdrressBytes,
      pool,
    };
  }

  describe("sendRequestNative", function () {
    it("Transfers tokens to Most", async () => {
      const { most, bazero } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const total_before = await bazero.totalSupply();
      await most.sendRequestNative(ALEPH_ACCOUNT, { value: TOKEN_AMOUNT });
      const after = await bazero.totalSupply();

      let delta = total_before - (after + BigInt(TOKEN_AMOUNT / 10 ** 6));

      expect(delta).to.be.lt(10n);
    });

    it("Emits correct event", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);

      function at_least_half(amount_out) {
        return amount_out >= BigInt(TOKEN_AMOUNT / 10 ** 6 / 2);
      }
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, { value: TOKEN_AMOUNT }),
      )
        .to.emit(most, "CrosschainTransferRequest")
        .withArgs(
          0,
          "0x0000000000000000000000000000000000000000000000000000000000000000",
          at_least_half,
          ALEPH_ACCOUNT,
          0,
        );
    });
  });

  describe("receiveRequest", function () {
    it("Reverts if caller is not a guardian", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );

      await expect(
        most
          .connect(accounts[0])
          .receiveRequest(
            requestHash,
            0,
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "NotInCommittee");
    });

    it("Ignores consecutive signatures", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );

      await most
        .connect(accounts[1])
        .receiveRequest(
          requestHash,
          0,
          bazeroAdrressBytes,
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
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      )
        .to.emit(most, "RequestAlreadySigned")
        .withArgs(requestHash, accounts[1].address);
    });

    it("Ignores already executed requests", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            0,
            bazeroAdrressBytes,
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
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      )
        .to.emit(most, "ProcessedRequestSigned")
        .withArgs(requestHash, accounts[6].address);
    });

    it("Unlocks tokens for the user", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );

      let balance_before = await ethers.provider.getBalance(
        accounts[10].address,
      );

      for (let i = 1; i < 6; i++) {
        await most
          .connect(accounts[i])
          .receiveRequest(
            requestHash,
            0,
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          );
      }

      let balance_after = await ethers.provider.getBalance(
        accounts[10].address,
      );

      expect(
        balance_after - (balance_before + BigInt(TOKEN_AMOUNT * 10 ** 6)),
      ).to.be.lt(10);
    });

    it("Reverts on non-matching hash", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHash = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 1],
      );

      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            requestHash,
            0,
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "DataHashMismatch");
    });

    it("Committee rotation", async () => {
      const { most, bazeroAdrressBytes } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[10].address);
      const requestHashOld = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [0, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );
      const requestHashNew = ethers.solidityPackedKeccak256(
        ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
        [1, bazeroAdrressBytes, TOKEN_AMOUNT, ethAddress, 0],
      );

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
          bazeroAdrressBytes,
          TOKEN_AMOUNT,
          ethAddress,
          0,
        );

      await most
        .connect(accounts[9])
        .receiveRequest(
          requestHashNew,
          1,
          bazeroAdrressBytes,
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
            bazeroAdrressBytes,
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
            bazeroAdrressBytes,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWithCustomError(most, "NotInCommittee");
    });
  });

  describe("Upgrade", function () {
    it("Most contract can be upgraded", async () => {
      exec(`cp ./contracts/MostL2.sol ./contracts/MostL2V2.sol`, (error) => {
        if (error !== null) {
          console.log("exec error: " + error);
        }
        exec(
          `sed -i "17 a     uint256 public test;" ./contracts/MostL2V2.sol`,
          async (error) => {
            if (error !== null) {
              console.log("exec error: " + error);
            }

            const { most, mostAddress, token, tokenAddressBytes32 } =
              await loadFixture(deployEightGuardianMostFixture);

            await token.approve(mostAddress, TOKEN_AMOUNT);

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
      exec(`rm ./contracts/MostL2V2.sol`, (error) => {
        if (error !== null) {
          console.log("exec error: " + error);
        }
      });
    });
  });
});
