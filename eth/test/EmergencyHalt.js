const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
  setNextBlockBaseFeePerGas,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");

// Import utils
const {
  addressToBytes32,
  getBytes32FromSeed,
  deployEightGuardianMostFixture,
  WRAPPED_TOKEN_ADDRESS,
  TOKEN_AMOUNT,
  ALEPH_ACCOUNT,
} = require("./TestUtils");

describe("EmergencyHalt", function () {
  describe("Halting works", function () {
    it("Halter can halt", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await expect(most.connect(accounts[10]).emergencyHalt()).not.to.be
        .reverted;
    });

    it("Non-halter cannot halt", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await expect(
        most.connect(accounts[0]).emergencyHalt(),
      ).to.be.revertedWith("NotEmergencyHalter");
    });

    it("Send request reverts if emergency halt has been performed", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();

      await expect(
        most.sendRequest(WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWith("EmergencyHalt");
    });

    it("Receive request reverts if emergency halt has been performed", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[15].address);

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();

      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            getBytes32FromSeed(13),
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWith("EmergencyHalt");
    });
  });

  describe("Halting stops working after its duration has elapsed", async () => {
    it("Send request is not blocked after emergency halt has expired", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();
      await ethers.provider.send("evm_increaseTime", [101]);

      await expect(
        most.sendRequest(WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWith("Unsupported pair");
    });

    it("Receive request is not blocked after emergency halt has expired", async () => {
      const { most, tokenAddressBytes32 } = await loadFixture(
        deployEightGuardianMostFixture,
      );
      const accounts = await ethers.getSigners();
      const ethAddress = addressToBytes32(accounts[15].address);

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();
      await ethers.provider.send("evm_increaseTime", [101]);

      await expect(
        most
          .connect(accounts[1])
          .receiveRequest(
            getBytes32FromSeed(13),
            tokenAddressBytes32,
            TOKEN_AMOUNT,
            ethAddress,
            0,
          ),
      ).to.be.revertedWith("Hash does not match the data");
    });
  });

  describe("Halting frequency", async () => {
    it("Halting cannot be performed more than once in a given period", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();
      await expect(
        most.connect(accounts[10]).emergencyHalt(),
      ).to.be.revertedWith("EmergencyHaltFrequency");
    });

    it("Halting can be performed after the period has elapsed", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();
      await ethers.provider.send("evm_increaseTime", [1001]);
      await expect(most.connect(accounts[10]).emergencyHalt()).not.to.be
        .reverted;
    });
  });

  describe("EmergencyHalter contract", async () => {
    async function deployEmergencyHalterFixture() {
      const { most, mostAddress } = await deployEightGuardianMostFixture();
      const EmergencyHalter =
        await ethers.getContractFactory("EmergencyHalter");
      const accounts = await ethers.getSigners();

      const emergencyHalter = await EmergencyHalter.deploy(
        [accounts[13], accounts[14]],
        mostAddress,
      );
      const emergencyHalterAddress = await emergencyHalter.getAddress();
      await most.setEmergencyHaltParams(emergencyHalterAddress, 100, 1000);

      return {
        most,
        emergencyHalter,
      };
    }

    it("Halter can halt", async () => {
      const { emergencyHalter } = await loadFixture(
        deployEmergencyHalterFixture,
      );
      const accounts = await ethers.getSigners();

      await expect(emergencyHalter.connect(accounts[13]).halt()).not.to.be
        .reverted;
    });

    it("Halter cannot halt if not in the list", async () => {
      const { emergencyHalter } = await loadFixture(
        deployEmergencyHalterFixture,
      );
      const accounts = await ethers.getSigners();

      await expect(emergencyHalter.connect(accounts[1]).halt()).to.be.reverted;
    });

    it("Owner can add halter", async () => {
      const { emergencyHalter } = await loadFixture(
        deployEmergencyHalterFixture,
      );
      const accounts = await ethers.getSigners();

      await expect(emergencyHalter.addHalter(accounts[1].address)).not.to.be
        .reverted;

      await expect(emergencyHalter.connect(accounts[1]).halt()).not.to.be
        .reverted;
    });

    it("Owner can remove halter", async () => {
      const { emergencyHalter } = await loadFixture(
        deployEmergencyHalterFixture,
      );
      const accounts = await ethers.getSigners();

      await expect(emergencyHalter.removeHalter(accounts[13].address)).not.to.be
        .reverted;

      await expect(emergencyHalter.connect(accounts[13]).halt()).to.be.reverted;
    });

    it("Integrates with Most contract", async () => {
      const { most, emergencyHalter } = await loadFixture(
        deployEmergencyHalterFixture,
      );
      const accounts = await ethers.getSigners();

      await expect(emergencyHalter.connect(accounts[13]).halt()).not.to.be
        .reverted;

      await expect(
        most.sendRequest(WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWith("EmergencyHalt");

      // wait for the halting period to elapse
      await ethers.provider.send("evm_increaseTime", [101]);

      await expect(
        most.sendRequest(WRAPPED_TOKEN_ADDRESS, TOKEN_AMOUNT, ALEPH_ACCOUNT),
      ).to.be.revertedWith("Unsupported pair");
    });
  });

  describe("Owner calls on Most contract", async () => {
    it("EmergencyHalt params can be set", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await expect(most.setEmergencyHaltParams(accounts[10], 100, 1000)).not.to
        .be.reverted;
    });

    it("EmergencyHalt params cannot be set by non-owner", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await expect(
        most
          .connect(accounts[1])
          .setEmergencyHaltParams(accounts[10], 100, 1000),
      ).to.be.reverted;
    });

    it("EmergencyHalter can be set to zero address", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);

      await expect(most.setEmergencyHaltParams(ethers.ZeroAddress, 100, 1000))
        .not.to.be.reverted;
    });

    it("Owner can clear the timestap", async () => {
      const { most } = await loadFixture(deployEightGuardianMostFixture);
      const accounts = await ethers.getSigners();

      await most.setEmergencyHaltParams(accounts[10], 100, 1000);
      await most.connect(accounts[10]).emergencyHalt();

      // Check that the timestamp has been set
      expect(await most.lastEmergencyHalt()).to.not.equal(0);

      await expect(most.resetEmergencyHaltTimestamp()).not.to.be.reverted;

      // Check that the timestamp has been reset
      expect(await most.lastEmergencyHalt()).to.equal(0);
    });
  });
});
