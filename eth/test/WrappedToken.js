const { expect } = require("chai");
const hre = require("hardhat");
const {
  loadFixture,
  setBalance,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");

const { ethToWei } = require("./TestUtils");

describe("Wrapped Token", function () {
  const SEED_AMOUNT = 1000n;
  const WRAP_AMOUNT = 100n;

  async function setupWrappedTokenFixture() {
    const [owner, minterBurner] = await hre.ethers.getSigners();
    const factory = await hre.ethers.getContractFactory("WrappedToken");
    const wrapped = await factory.deploy("Wrapped Token", "WTK", 8, minterBurner.address);
    await setBalance(owner.address, ethToWei(SEED_AMOUNT));

    return { wrapped, owner, minterBurner };
  }

  describe("Preliminaries", function () {
    it("Initial supply", async function () {
      const { wrapped } = await loadFixture(setupWrappedTokenFixture);
      expect(await wrapped.totalSupply()).to.equal(0n);
    });

    it("Token name", async function () {
      const { wrapped } = await loadFixture(setupWrappedTokenFixture);
      expect(await wrapped.name()).to.equal("Wrapped Token");
    });

    it("Token symbol", async function () {
      const { wrapped } = await loadFixture(setupWrappedTokenFixture);
      expect(await wrapped.symbol()).to.equal("WTK");
    });

    it("Decimals", async function () {
      const { wrapped } = await loadFixture(setupWrappedTokenFixture);
      expect(await wrapped.decimals()).to.equal(8);
    });
  });

  describe("Ownership", function () {
    it("Owner can change MinterBurner.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedTokenFixture);
      expect(await wrapped.connect(owner).setMinterBurner(owner.address)).to.not.be.reverted;
      expect(await wrapped.minterBurner()).to.equal(owner.address);
    });

    it("Non-owner cannot change MinterBurner.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedTokenFixture);
      const [_, __, nonOwner] = await hre.ethers.getSigners();
      expect(wrapped.connect(nonOwner).setMinterBurner(owner.address)).to.be.reverted;
    });
  });

  describe("Minting", function () {
    it("MinterBurner can mint.", async function () {
      const { wrapped, minterBurner } = await loadFixture(setupWrappedTokenFixture);
      const [_, __, nonOwner] = await hre.ethers.getSigners();
      await expect(wrapped.connect(minterBurner).mint(nonOwner.address, WRAP_AMOUNT)).to.not.be.reverted;
      expect(await wrapped.totalSupply()).to.equal(WRAP_AMOUNT);
      expect(await wrapped.balanceOf(nonOwner.address)).to.equal(WRAP_AMOUNT);
    });

    it("Non-MinterBurner cannot mint.", async function () {
      const { wrapped } = await loadFixture(setupWrappedTokenFixture);
      const [_, __, nonOwner] = await hre.ethers.getSigners();
      await expect(wrapped.mint(nonOwner.address, WRAP_AMOUNT)).to.be.reverted;
    });
  });

  describe("Burning", function () {
    it("MinterBurner can burn.", async function () {
      const { wrapped, minterBurner } = await loadFixture(setupWrappedTokenFixture);
      const [_, __, nonOwner] = await hre.ethers.getSigners();
      await wrapped.connect(minterBurner).mint(nonOwner.address, WRAP_AMOUNT);
      await expect(wrapped.connect(minterBurner).burn(WRAP_AMOUNT)).to.be.reverted;

      // Allow minterBurner to take tokens from nonOwner, transfer to itself, and burn.
      await wrapped.connect(nonOwner).approve(minterBurner.address, WRAP_AMOUNT);
      await wrapped.connect(minterBurner).transferFrom(nonOwner.address, minterBurner.address, WRAP_AMOUNT);
      await expect(wrapped.connect(minterBurner).burn(WRAP_AMOUNT)).to.not.be.reverted;

      expect(await wrapped.totalSupply()).to.equal(0n);
      expect(await wrapped.balanceOf(nonOwner.address)).to.equal(0n);
    });

    it("Non-MinterBurner cannot burn.", async function () {
      const { wrapped, minterBurner } = await loadFixture(setupWrappedTokenFixture);
      // Mint tokens to nonOwner account.
      const [_, __, nonOwner] = await hre.ethers.getSigners();
      await wrapped.connect(minterBurner).mint(nonOwner.address, WRAP_AMOUNT);
      await expect(wrapped.connect(nonOwner).burn(WRAP_AMOUNT)).to.be.reverted;
    });
  });
});
