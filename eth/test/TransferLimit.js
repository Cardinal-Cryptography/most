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
  ethToWei,
} = require("./TestUtils");
const { fail } = require("assert");

const ALEPH_ACCOUNT = getRandomAlephAccount(13);
const WRAPPED_TOKEN_ADDRESS = getRandomAlephAccount(15);
const WRAPPED_WETH_ADDRESS = getRandomAlephAccount(16);
const WAZERO_ADDRESS = getRandomAlephAccount(17);
const WETH_DECIMALS = 18;
const TOKEN_DECIMALS = 15;
const WAZERO_DECIMALS = 12;

async function setup() {
  const most = await setupMostFixture();
  const limit = await setupTransferLimitFixture(most);
  const oracle = await setupOracleFixture();
  const weth = await most.wethAddress();
  const token = await setupTokenFixture(most);
  const wazero = await setupWazeroFixture(most);

  await most.unpause();
  const tokenAddress = await token.getAddress();
  const mostAddress = await most.getAddress();
  const tokenAddressBytes32 = addressToBytes32(tokenAddress);
  const wazeroAddress = await wazero.getAddress();

  return {
    most,
    mostAddress,
    limit,
    oracle,
    weth,
    token,
    tokenAddress,
    tokenAddressBytes32,
    wazero,
    wazeroAddress,
  };
}

async function setupWazeroFixture(most) {
  const Wazero = await ethers.getContractFactory("WrappedToken");
  const wazero = await Wazero.deploy(
    "Wrapped AZERO",
    "wAZERO",
    WAZERO_DECIMALS,
    await most.getAddress()
  );
  const wrappedAzeroAddress = await wazero.getAddress();
  const wrappedAzeroAddressBytes32 = addressToBytes32(wrappedAzeroAddress);

  await mintPSP22ToSelf(wazero, 1000 * 10 ** WAZERO_DECIMALS);
  await most.setWrappedAzeroAddress(wrappedAzeroAddress);
  await most.addPair(wrappedAzeroAddressBytes32, WAZERO_ADDRESS, false);

  return wazero;
}

async function mintPSP22ToSelf(wrappedAzero, amount) {
  let currentMinterBurner = await wrappedAzero.minterBurner();
  const accounts = await ethers.getSigners();
  let account = accounts[0];
  await wrappedAzero.setMinterBurner(account);
  await wrappedAzero.mint(account, amount);
  await wrappedAzero.setMinterBurner(currentMinterBurner);
}

async function setupTokenFixture(most) {
  const Token = await ethers.getContractFactory("Token");
  const token = await Token.deploy(
    "10000000000000000000000000",
    "15",
    "TestToken",
    "TEST"
  );

  const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
  await most.addPair(tokenAddressBytes32, WRAPPED_TOKEN_ADDRESS, true);

  return token;
}

async function setupMostFixture() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  const WETH = await ethers.getContractFactory("WETH9");
  const weth = await WETH.deploy();

  const Most = await ethers.getContractFactory("Most");
  const most = await upgrades.deployProxy(
    Most,
    [[accounts[0]], 1, accounts[0], await weth.getAddress()],
    {
      initializer: "initialize",
      kind: "uups",
    }
  );

  const wethAddressBytes32 = addressToBytes32(await weth.getAddress());
  await most.addPair(wethAddressBytes32, WRAPPED_WETH_ADDRESS, true);

  return most;
}

async function setupTransferLimitFixture(most) {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  const TransferLimit = await ethers.getContractFactory("TransferLimit");
  const limit = await upgrades.deployProxy(TransferLimit, [accounts[0]], {
    initializer: "initialize",
    kind: "uups",
  });

  most.setTransferLimit(await limit.getAddress());

  return limit;
}

async function setupOracleFixture() {
  const Oracle = await ethers.getContractFactory("MockOracle");
  const oracle = await Oracle.deploy();

  return oracle;
}

describe("TransferLimit", function () {
  describe("Limit for WETH transfers", function () {
    it("Does not enforce a limit if not configured", async () => {
      const most = await setupMostFixture();
      await most.unpause();

      await expect(most.sendRequestNative(ALEPH_ACCOUNT, { value: 1 })).to.not
        .be.reverted;
    });

    it("Does not enforce a limit if the limit is set to 0 and no oracle", async () => {
      const { most, limit, weth } = await setup();

      expect(await limit.minimumTransferAmount(weth)).to.equal(0);
      await expect(most.sendRequestNative(ALEPH_ACCOUNT, { value: 1 })).to.not
        .be.reverted;
    });

    it("Respects the default limit", async () => {
      const { most, limit, weth } = await setup();

      await limit.setDefaultLimit(weth, 100);

      expect(await limit.minimumTransferAmount(weth)).to.equal(100);
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, { value: 10 })
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, { value: 100 })
      ).to.not.be.revertedWithCustomError(most, "LimitExceeded");
    });

    it("Respects the oracle limit", async () => {
      const { most, limit, oracle, weth } = await setup();

      await limit.setUSDOracle(
        weth,
        WETH_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      await limit.setDefaultLimit(weth, BigInt(1000 * 10 ** WETH_DECIMALS));
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);

      const expectedMinimumAmount =
        BigInt(1 * 10 ** WETH_DECIMALS) / BigInt(50);
      expect(await limit.minimumTransferAmount(weth)).to.equal(
        expectedMinimumAmount
      );
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, {
          value: expectedMinimumAmount - BigInt(1),
        })
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, {
          value: expectedMinimumAmount,
        })
      ).to.not.be.reverted;
    });

    it("Respects the default limit if it's lower than the oracle limit", async () => {
      const { most, limit, oracle, weth } = await setup();

      await limit.setUSDOracle(
        weth,
        WETH_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      const defaultLimit = BigInt((1 * 10 ** WETH_DECIMALS) / 1000);
      await limit.setDefaultLimit(weth, defaultLimit);
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);

      expect(await limit.minimumTransferAmount(weth)).to.equal(defaultLimit);
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, {
          value: defaultLimit - BigInt(1),
        })
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequestNative(ALEPH_ACCOUNT, {
          value: defaultLimit,
        })
      ).to.not.be.reverted;
    });
  });

  describe("Limit for token trasnfers", function () {
    it("Does not enforce a limit if not configured", async () => {
      const most = await setupMostFixture();
      const mostAddress = await most.getAddress();
      const token = await setupTokenFixture(most);
      const tokenAddressBytes32 = addressToBytes32(await token.getAddress());
      await most.unpause();

      await token.approve(mostAddress, 1);

      await expect(most.sendRequest(tokenAddressBytes32, 1, ALEPH_ACCOUNT)).to
        .not.be.reverted;
    });

    it("Does not enforce a limit if the limit is set to 0 and no oracle", async () => {
      const { most, token, mostAddress, tokenAddressBytes32 } = await setup();

      await token.approve(mostAddress, 1);

      await expect(most.sendRequest(tokenAddressBytes32, 1, ALEPH_ACCOUNT)).to
        .not.be.reverted;
    });

    it("Respects the default limit", async () => {
      const {
        most,
        mostAddress,
        limit,
        token,
        tokenAddress,
        tokenAddressBytes32,
      } = await setup();

      await limit.setDefaultLimit(tokenAddress, 100);
      await token.approve(mostAddress, 100);

      expect(await limit.minimumTransferAmount(tokenAddress)).to.equal(100);
      await expect(
        most.sendRequest(tokenAddressBytes32, 10, ALEPH_ACCOUNT)
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(most.sendRequest(tokenAddressBytes32, 100, ALEPH_ACCOUNT)).to
        .not.be.reverted;
    });

    it("Respects the oracle limit", async () => {
      const {
        most,
        mostAddress,
        limit,
        oracle,
        token,
        tokenAddress,
        tokenAddressBytes32,
      } = await setup();

      await limit.setUSDOracle(
        token,
        TOKEN_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      await limit.setDefaultLimit(
        tokenAddress,
        BigInt(1000 * 10 ** TOKEN_DECIMALS)
      );
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);
      const expectedMinimumAmount =
        BigInt(1 * 10 ** TOKEN_DECIMALS) / BigInt(50);
      await token.approve(mostAddress, expectedMinimumAmount);

      expect(await limit.minimumTransferAmount(tokenAddress)).to.equal(
        expectedMinimumAmount
      );
      await expect(
        most.sendRequest(
          tokenAddressBytes32,
          expectedMinimumAmount - BigInt(1),
          ALEPH_ACCOUNT
        )
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequest(
          tokenAddressBytes32,
          expectedMinimumAmount,
          ALEPH_ACCOUNT
        )
      ).to.not.be.reverted;
    });

    it("Respects the default limit if it's lower than the oracle limit", async () => {
      const {
        most,
        mostAddress,
        limit,
        oracle,
        token,
        tokenAddress,
        tokenAddressBytes32,
      } = await setup();

      await limit.setUSDOracle(
        token,
        TOKEN_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      const defaultLimit = BigInt((1 * 10 ** TOKEN_DECIMALS) / 1000);
      await limit.setDefaultLimit(tokenAddress, defaultLimit);
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);
      await token.approve(mostAddress, defaultLimit);

      expect(await limit.minimumTransferAmount(tokenAddress)).to.equal(
        defaultLimit
      );
      await expect(
        most.sendRequest(
          tokenAddressBytes32,
          defaultLimit - BigInt(1),
          ALEPH_ACCOUNT
        )
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequest(tokenAddressBytes32, defaultLimit, ALEPH_ACCOUNT)
      ).to.not.be.reverted;
    });
  });

  describe("Limit for AZERO transfers", function () {
    it("Does not enforce a limit if not configured", async () => {
      const most = await setupMostFixture();
      const mostAddress = await most.getAddress();
      const wazero = await setupWazeroFixture(most);

      await most.unpause();
      await wazero.approve(mostAddress, 1);

      await expect(most.sendRequestAzeroToNative(1, ALEPH_ACCOUNT)).to.not.be
        .reverted;
    });

    it("Does not enforce a limit if the limit is set to 0 and no oracle", async () => {
      const { most, mostAddress, wazero } = await setup();

      await wazero.approve(mostAddress, 1);

      await expect(most.sendRequestAzeroToNative(1, ALEPH_ACCOUNT)).to.not.be
        .reverted;
    });

    it("Respects the default limit", async () => {
      const { most, mostAddress, limit, wazero, wazeroAddress } = await setup();

      await limit.setDefaultLimit(wazeroAddress, 100);
      await wazero.approve(mostAddress, 100);

      expect(await limit.minimumTransferAmount(wazeroAddress)).to.equal(100);
      await expect(
        most.sendRequestAzeroToNative(10, ALEPH_ACCOUNT)
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(most.sendRequestAzeroToNative(100, ALEPH_ACCOUNT)).to.not.be
        .reverted;
    });

    it("Respects the oracle limit", async () => {
      const { most, mostAddress, limit, oracle, wazero, wazeroAddress } =
        await setup();

      await limit.setUSDOracle(
        wazeroAddress,
        WAZERO_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      await limit.setDefaultLimit(
        wazeroAddress,
        BigInt(1000 * 10 ** WAZERO_DECIMALS)
      );
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);
      const expectedMinimumAmount =
        BigInt(1 * 10 ** WAZERO_DECIMALS) / BigInt(50);
      await wazero.approve(mostAddress, expectedMinimumAmount);

      expect(await limit.minimumTransferAmount(wazeroAddress)).to.equal(
        expectedMinimumAmount
      );
      await expect(
        most.sendRequestAzeroToNative(
          expectedMinimumAmount - BigInt(1),
          ALEPH_ACCOUNT
        )
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(
        most.sendRequestAzeroToNative(expectedMinimumAmount, ALEPH_ACCOUNT)
      ).to.not.be.reverted;
    });

    it("Respects the default limit if it's lower than the oracle limit", async () => {
      const { most, mostAddress, limit, oracle, wazero, wazeroAddress } =
        await setup();

      await limit.setUSDOracle(
        wazero,
        WAZERO_DECIMALS,
        await oracle.getAddress(),
        2
      );

      const oracleDecimals = await oracle.decimals();
      const defaultLimit = BigInt((1 * 10 ** WAZERO_DECIMALS) / 1000);
      await limit.setDefaultLimit(wazeroAddress, defaultLimit);
      await oracle.setPrice(BigInt(100) * BigInt(10) ** oracleDecimals);
      await wazero.approve(mostAddress, defaultLimit);

      expect(await limit.minimumTransferAmount(wazeroAddress)).to.equal(
        defaultLimit
      );
      await expect(
        most.sendRequestAzeroToNative(defaultLimit - BigInt(1), ALEPH_ACCOUNT)
      ).to.be.revertedWithCustomError(most, "LimitExceeded");
      await expect(most.sendRequestAzeroToNative(defaultLimit, ALEPH_ACCOUNT))
        .to.not.be.reverted;
    });
  });
});
