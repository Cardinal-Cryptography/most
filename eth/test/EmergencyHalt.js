const { expect } = require("chai");
const { ethers, upgrades } = require("hardhat");
const {
  loadFixture,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");

// Import fixtures
const { deployEightGuardianMostFixture } = require("../fixtures");

// Import utils
const { getRandomAlephAccount } = require("./TestUtils");

const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(5);

describe("EmergencyHalt", function () {
  describe("sendRequest", function () {
    it("Reverts if the USD value of the transfer amount is below the minimum", async () => {
      const { most, tokenAddressBytes32, usdtAddressBytes32 } =
        await loadFixture(deployEightGuardianMostFixture);

      await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS);
    });
  });
});
