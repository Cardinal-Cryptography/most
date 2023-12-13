const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");

// Import utils
const { getRandomAlephAccount, deployEightGuardianMostFixture } = require("./TestUtils");

const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);

describe("EmergencyHalt", function () {
  describe("Halting works", function () {
    it("Send request reverts if emergency halt has been performed", async () => {
      const { most, tokenAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);
    });
    it("Receive request reverts if emergency halt has been performed", async () => {
      const { most, tokenAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);
    });
  });
  describe("Halting stops working after its duration has elapsed", async () => {
    it("Senf request succeeds after emergency halt has expired", async () => {
      const { most, tokenAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);
    });
    it("Receive request succeeds after emergency halt has expired", async () => {
      const { most, tokenAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);
    });
  });
  describe("Owner ops on EmergencyHalter contract", async () => {});
});
