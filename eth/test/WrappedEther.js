const { expect } = require("chai");
const hre = require("hardhat");
const { loadFixture, setBalance } = require("@nomicfoundation/hardhat-toolbox/network-helpers");
const {min} = require("hardhat/internal/util/bigint");

describe("Wrapped Ether", function () {
    const DECIMALS = 10n ** 18n;
    const SEED_AMOUNT = 1000n;
    const WRAP_AMOUNT = 100n;

    async function setupWrappedEtherFixture() {
        const [owner] = await hre.ethers.getSigners();
        const factory = await hre.ethers.getContractFactory("WrappedEther");
        const wrapped = await factory.deploy();
        await setBalance(owner.address, SEED_AMOUNT * DECIMALS);

        return { wrapped, owner };
    }

    it("Initial Wrapped Ether supply should be zero.", async function () {
        const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

        const ownerBalance = await wrapped.balanceOf(owner.address);
        expect(await wrapped.totalSupply()).to.equal(ownerBalance);
        expect(await wrapped.totalSupply()).to.equal(0n);
    });

    it("Wrapped amount should be equal to the transferred amount.", async function () {
        const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

        await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString())});
        expect(await wrapped.balanceOf(owner.address)).to.equal(WRAP_AMOUNT * DECIMALS);
    });

    it("Emits mint event.", async function () {
        const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

        await expect(wrapped.mint({
            value: hre.ethers.parseEther(WRAP_AMOUNT.toString())
        })).to.emit(wrapped, "Mint").withArgs(
            owner.address,
            WRAP_AMOUNT * DECIMALS,
        );
    });

    it("Reverts when trying to burn more tokens then are available.", async function () {
        const { wrapped, _owner } = await loadFixture(setupWrappedEtherFixture);
        await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });

        await expect(wrapped.burn(WRAP_AMOUNT * DECIMALS + 1n)).to.be.revertedWith("ERC20: burn amount exceeds balance");
    });

    //it("Unwrapped amount should be equal to the transferred amount.", async function () {
    //    const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
    //    const provider = hre.ethers.provider;
    //    const balance_init = await provider.getBalance(owner.address);
    //    expect(balance_init).to.equal(SEED_AMOUNT * DECIMALS);

    //    const balance = await provider.getBalance(owner.address);
    //    expect(await wrapped.balanceOf(owner.address)).to.equal(0n);
    //    expect(balance).to.equal(SEED_AMOUNT * DECIMALS - (mintGasEstimate + burnGasEstimate));
    //});

    it("Emits burn event.", async function () {
        const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
        await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });

        await expect(wrapped.burn(WRAP_AMOUNT * DECIMALS)).to.emit(wrapped, "Burn").withArgs(
            owner.address,
            WRAP_AMOUNT * DECIMALS,
        );
    });
});