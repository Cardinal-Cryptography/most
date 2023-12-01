const { expect } = require("chai");
const hre = require("hardhat");
const {
  loadFixture,
  setBalance,
} = require("@nomicfoundation/hardhat-toolbox/network-helpers");

const { ethToWei } = require("./TestUtils");

describe("Wrapped Ether", function () {
  const SEED_AMOUNT = 1000n;
  const WRAP_AMOUNT = 100n;

  async function setupWrappedEtherFixture() {
    const [owner] = await hre.ethers.getSigners();
    const factory = await hre.ethers.getContractFactory("WETH9");
    const wrapped = await factory.deploy();
    await setBalance(owner.address, ethToWei(SEED_AMOUNT));

    return { wrapped, owner };
  }

  describe("Preliminaries", function () {
    it("Initial Wrapped Ether supply should be zero.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

      const ownerBalance = await wrapped.balanceOf(owner);
      expect(await wrapped.totalSupply()).to.equal(ownerBalance);
      expect(await wrapped.totalSupply()).to.equal(0n);
    });
  });

  describe("Deposit", function () {
    it("Wrapped amount should be equal to the transferred amount.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

      await expect(
        wrapped.deposit({ value: ethToWei(WRAP_AMOUNT) }),
      ).to.changeTokenBalance(wrapped, owner, ethToWei(WRAP_AMOUNT));
    });

    it("Emits deposit event.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

      await expect(
        wrapped.deposit({
          value: ethToWei(WRAP_AMOUNT),
        }),
      )
        .to.emit(wrapped, "Deposit")
        .withArgs(owner.address, ethToWei(WRAP_AMOUNT));
    });
  });

  describe("Withdraw", function () {
    it("Reverts when trying to withdraw more tokens then are available.", async function () {
      const { wrapped, _owner } = await loadFixture(setupWrappedEtherFixture);
      await wrapped.deposit({ value: ethToWei(WRAP_AMOUNT) });

      await expect(wrapped.withdraw(ethToWei(WRAP_AMOUNT) + 1n)).to.be.reverted;
    });

    it("Emits withdraw event.", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
      await wrapped.deposit({ value: ethToWei(WRAP_AMOUNT) });

      await expect(wrapped.withdraw(ethToWei(WRAP_AMOUNT)))
        .to.emit(wrapped, "Withdrawal")
        .withArgs(owner.address, ethToWei(WRAP_AMOUNT));
    });
  });

  describe("Round trip", function () {
    it("No wrapped tokens left after unwrapping the whole balance", async function () {
      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
      await wrapped.deposit({ value: ethToWei(WRAP_AMOUNT) });

      await expect(
        wrapped.withdraw(ethToWei(WRAP_AMOUNT)),
      ).to.changeTokenBalance(wrapped, owner, ethToWei(-WRAP_AMOUNT));
      expect(await wrapped.balanceOf(owner)).to.equal(0n);
    });

    it("Unwrapped amount should be equal to the transferred amount.", async function () {
      // We assume a tolerance of 10 ** (-4) ETH or 10 ** 14 WEI.
      const TOLERANCE = 10 ** 14;

      const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
      const provider = hre.ethers.provider;
      const balance_init = await provider.getBalance(owner);

      const depositGasEstimate = await wrapped.deposit.estimateGas({
        value: ethToWei(WRAP_AMOUNT),
      });
      await wrapped.deposit({ value: ethToWei(WRAP_AMOUNT) });
      const balance_after_deposit = await provider.getBalance(owner);
      expect(
        balance_init -
          balance_after_deposit -
          ethToWei(WRAP_AMOUNT) -
          depositGasEstimate,
      ).to.be.lessThan(TOLERANCE.toString());

      const withdrawGasEstimate = await wrapped.withdraw.estimateGas(
        ethToWei(WRAP_AMOUNT),
      );
      await wrapped.withdraw(ethToWei(WRAP_AMOUNT));
      const balance_after_withdraw = await provider.getBalance(owner);
      expect(
        balance_after_withdraw -
          balance_after_deposit -
          ethToWei(WRAP_AMOUNT) -
          withdrawGasEstimate,
      ).to.be.lessThan(TOLERANCE.toString());

      expect(
        balance_after_withdraw -
          balance_init -
          depositGasEstimate -
          withdrawGasEstimate,
      ).to.be.lessThan(TOLERANCE.toString());
    });
  });
});
