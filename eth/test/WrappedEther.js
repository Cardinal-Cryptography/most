const { expect } = require("chai");
const hre = require("hardhat");
const { loadFixture, setBalance } = require("@nomicfoundation/hardhat-toolbox/network-helpers");

describe("Wrapped Ether", function () {
    const DECIMALS = 10n ** 18n;
    const SEED_AMOUNT = 1000n;
    const WRAP_AMOUNT = 100n;

    async function setupWrappedEtherFixture() {
        const [owner] = await hre.ethers.getSigners();
        const factory = await hre.ethers.getContractFactory("WrappedEther");
        const wrapped = await factory.deploy();
        await setBalance(owner.address, hre.ethers.parseEther(SEED_AMOUNT.toString()));

        return { wrapped, owner };
    }

    describe("Preliminaries", function () {
        it("Initial Wrapped Ether supply should be zero.", async function () {
            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

            const ownerBalance = await wrapped.balanceOf(owner.address);
            expect(await wrapped.totalSupply()).to.equal(ownerBalance);
            expect(await wrapped.totalSupply()).to.equal(0n);
        });
    });

    describe("Mint", function () {
        it("Wrapped amount should be equal to the transferred amount.", async function () {
            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

            await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString())});
            expect(await wrapped.balanceOf(owner.address)).to.equal(hre.ethers.parseEther(WRAP_AMOUNT.toString()));
        });

        it("Emits mint event.", async function () {
            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);

            await expect(wrapped.mint({
                value: hre.ethers.parseEther(WRAP_AMOUNT.toString())
            })).to.emit(wrapped, "Mint").withArgs(
                owner.address,
                hre.ethers.parseEther(WRAP_AMOUNT.toString()),
            );
        });
    });

    describe("Burn", function () {
        it("Reverts when trying to burn more tokens then are available.", async function () {
            const { wrapped, _owner } = await loadFixture(setupWrappedEtherFixture);
            await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });

            await expect(wrapped.burn(hre.ethers.parseEther(WRAP_AMOUNT.toString()) + 1n)).to.be.revertedWith("ERC20: burn amount exceeds balance");
        });

        it("Emits burn event.", async function () {
            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
            await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });

            await expect(wrapped.burn(WRAP_AMOUNT * DECIMALS)).to.emit(wrapped, "Burn").withArgs(
                owner.address,
                WRAP_AMOUNT * DECIMALS,
            );
        });
    });

    describe("Round trip", function () {
        it("No wrapped tokens left after unwrapping the whole balance", async function () {
            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
            const provider = hre.ethers.provider;
            const balance_init = await provider.getBalance(owner.address);
            expect(balance_init).to.equal(hre.ethers.parseEther(SEED_AMOUNT.toString()));
            await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });

            await wrapped.burn(hre.ethers.parseEther(WRAP_AMOUNT.toString()));
            expect(await wrapped.balanceOf(owner.address)).to.equal(0n);
        });

        it("Unwrapped amount should be equal to the transferred amount.", async function () {
            // We assume a tolerance of 10 ** (-4) ETH or 10 ** 14 WEI.
            const TOLERANCE = 10 ** 14;

            const { wrapped, owner } = await loadFixture(setupWrappedEtherFixture);
            const provider = hre.ethers.provider;
            const balance_init = await provider.getBalance(owner.address);

            const mintGasEstimate = await wrapped.mint.estimateGas({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });
            await wrapped.mint({ value: hre.ethers.parseEther(WRAP_AMOUNT.toString()) });
            const balance_after_mint = await provider.getBalance(owner.address);
            expect(
                balance_init - balance_after_mint - hre.ethers.parseEther(WRAP_AMOUNT.toString()) - mintGasEstimate
            ).to.be.lessThan(TOLERANCE.toString());

            const burnGasEstimate = await wrapped.burn.estimateGas(hre.ethers.parseEther(WRAP_AMOUNT.toString()));
            await wrapped.burn(hre.ethers.parseEther(WRAP_AMOUNT.toString()));
            const balance_after_burn = await provider.getBalance(owner.address);
            expect(
                balance_after_burn - balance_after_mint - hre.ethers.parseEther(WRAP_AMOUNT.toString()) - burnGasEstimate
            ).to.be.lessThan(TOLERANCE.toString());

            expect(
                balance_after_burn - balance_init - mintGasEstimate - burnGasEstimate
            ).to.be.lessThan(TOLERANCE.toString());
        });
    });
});