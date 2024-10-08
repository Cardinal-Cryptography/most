const { ethers, network } = require("hardhat");
const fs = require("node:fs");
const { u8aToHex } = require("@polkadot/util");
const { Keyring } = require("@polkadot/keyring");
const NATIVE_ADDRESS = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

async function assert_signer_has_funds(signer, config) {
  const needed = tokenToRaw(config.pool_config.initial_native_supply, 18);
  const balance = await ethers.provider.getBalance(signer);

  if (needed > balance) {
    console.log(
      `The account ${signer} does not have enough funds.
      At least ${needed} is needed.
      Balance: ${balance}`,
    );
    throw "Not enough funds";
  }
}

async function load_contracts() {
  const { lp, pool, most, bazero } = JSON.parse(
    fs.readFileSync("l2_addresses.json", { encoding: "utf8", flag: "r" }),
  );

  const WrappedToken = await ethers.getContractFactory("WrappedToken");
  const LP = await ethers.getContractFactory("StableSwapLP");
  const Pool = await ethers.getContractFactory("StableSwapTwoPool");
  const MostL2 = await ethers.getContractFactory("MostL2");

  const bazeroContract = WrappedToken.attach(bazero);
  const lpContract = LP.attach(lp);
  const poolContract = Pool.attach(pool);
  const mostContract = MostL2.attach(most);

  return [bazeroContract, lpContract, poolContract, mostContract];
}

async function setMinterBurnerTo(contract, owner) {
  console.log(`Sets minter of ${contract.target} to ${owner}`);
  await contract.setMinterBurner(owner);
}
async function setMinterTo(contract, owner) {
  console.log(`Sets minter of ${contract.target} to ${owner}`);
  await contract.setMinter(owner);
}

function tokenToRaw(amount, decimals) {
  return BigInt(amount) * BigInt(10 ** decimals);
}

async function mintInitialBazero(bazero, to, config) {
  const mintAmount = tokenToRaw(
    config.bazero_token_mint,
    config.bazero_decimals,
  );

  console.log(`Minting ${mintAmount} of Bazero to account ${to}`);
  await bazero.mint(to, mintAmount);
}

async function initializePool(config, pool, lpAddress, bazero, owner) {
  const {
    amplification_coefficient,
    admin_fee,
    fee,
    initial_native_supply,
    initial_token_supply,
  } = config.pool_config;

  console.log(`Initializing SwapPool between ${bazero.target} <-> Native`);
  console.log(
    `Setting amplification coefficient to: ${amplification_coefficient}`,
  );
  console.log(`                        admin fee to: ${admin_fee}`);
  console.log(`                              fee to: ${fee}`);
  const initializeTx = await pool.initialize(
    [bazero.target, NATIVE_ADDRESS],
    amplification_coefficient,
    fee,
    admin_fee,
    owner,
    lpAddress,
  );
  await initializeTx.wait(1);

  console.log(`Approving spending of ${initial_token_supply} to pool`);
  const bazero_amount = tokenToRaw(
    initial_token_supply,
    config.bazero_decimals,
  );
  const native_amount = tokenToRaw(initial_native_supply, 18);
  await bazero.approve(pool.target, bazero_amount);

  console.log(
    `Providing pool with initial liquidity, transferring ${initial_token_supply} of Bazero tokens and ${initial_native_supply} of native currency`,
  );
  await pool.add_liquidity([bazero_amount, native_amount], 0, {
    value: native_amount,
  });
}

async function addTokenPair(
  ethTokenAddress,
  azeroTokenAddress,
  isLocal,
  mostContract,
  ownerSigner,
) {
  const ethTokenAddressBytes = ethers.zeroPadValue(
    ethers.getBytes(ethTokenAddress),
    32,
  );
  const azeroTokenAddressBytes = u8aToHex(
    new Keyring({ type: "sr25519" }).decodeAddress(azeroTokenAddress),
  );

  console.log(
    "Adding token pair to Most:",
    ethTokenAddress,
    "=>",
    azeroTokenAddress,
    "( direction:",
    isLocal ? "ETH -> Aleph" : "Aleph -> ETH",
    ")",
  );

  const addPairTx = await mostContract.addPair(
    ethTokenAddressBytes,
    azeroTokenAddressBytes,
    isLocal,
    {
      from: ownerSigner,
    },
  );
  await addPairTx.wait(1);

  console.log(
    "Most now supports the token pair:",
    ethTokenAddressBytes,
    "=>",
    await mostContract.supportedPairs(ethTokenAddressBytes),
  );
}

/// 1. Mint Bazero.
/// 2. Sets minter for LpToken to Pool.
/// 3. Sets minter for bazero to Most.
/// 4. Initialize pool, providing it with some initial liquidity.
/// 5. Unpause the most.
async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);
  const deployer_evm = accounts[0];

  console.log(`Using EVM deployer account: ${deployer_evm}`);

  const config = network.config.deploymentConfig;

  const [bazero, lp, pool, most] = await load_contracts();

  await assert_signer_has_funds(deployer_evm, config);

  await addTokenPair(
    bazero.target,
    config.wazero_l1_address,
    false,
    most,
    deployer_evm,
  );

  await mintInitialBazero(bazero, deployer_evm, config);

  await setMinterTo(lp, pool.target);
  await setMinterBurnerTo(bazero, most.target);

  await initializePool(config, pool, lp.target, bazero, deployer_evm);

  if (config.dev) {
    console.log("Unpause the most...");
    await most.unpause();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
