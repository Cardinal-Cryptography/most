const { ethers, upgrades, network } = require("hardhat");

/// Setups L2 most with a stable swap.
/// 1. deploy BAzero as a WrappedToken Contract with minter set to runner of this script
/// 2. deploy LpToken for internal Pool usage
/// 3. deploy StableSwap, sets minter of LpToken to the StableSwap address
/// 4. deploy MostL2,
///    i) mints 1M bazero to the runner of this script
///    ii) set minter of Bazero to the MostL2 contract
/// 5. add liquidity to the pool, 1k tokens both native and bazero.
/// 6. add pair between bazero <-> bazero todo: (the second end is a stub for now)
/// 7. unpause bridge and call sendRequestNative and receiveRequest.

async function main() {
  const signers = await ethers.getSigners();
  const accounts = signers.map((s) => s.address);

  const config = network.config.deploymentConfig;

  const WrappedToken = await ethers.getContractFactory("WrappedToken");
  const bazero = await WrappedToken.deploy(
    "Bridge Azero",
    "bazero",
    12,
    accounts[0],
  );
  await bazero.waitForDeployment();
  console.log("bazero deployed to:", bazero.target);

  const nativeToken = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

  const LP = await ethers.getContractFactory("StableSwapLP");
  const lp = await LP.deploy();
  await lp.waitForDeployment();

  console.log("LP deployed to:", lp.target);
  const Pool = await ethers.getContractFactory("StableSwapTwoPool");
  const pool = await Pool.deploy();
  await pool.waitForDeployment();
  console.log("Stable Swap deployed to:", pool.target);

  const setMinterTX = await lp.setMinter(pool.target);
  await setMinterTX.wait(1);

  const MostL2 = await ethers.getContractFactory("MostL2");
  console.log("Deploying MostL2...");
  const most = await upgrades.deployProxy(
    MostL2,
    [
      config.guardianIds,
      config.threshold,
      accounts[0],
      pool.target,
      bazero.target,
    ],
    {
      initializer: "initialize",
      kind: "uups",
    },
  );
  await most.waitForDeployment();
  console.log("MostL2 deployed to:", most.target);

  await bazero.mint(accounts[0], 1000000n * 10n ** 12n);
  await bazero.setMinterBurner(most.target);
  console.log("Bazero minter set to most");

  await bazero.approve(pool.target, 1000n * 10n ** 12n);

  const initializeTx = await pool.initialize(
    [bazero.target, nativeToken],
    1, // Todo set constants, fees etc
    1,
    1,
    accounts[0],
    lp.target,
  );
  await initializeTx.wait(1);

  // Todo: Next calls are for hand testing and setting up
  await pool.add_liquidity([1000n * 10n ** 12n, 1000n * 10n ** 18n], 0, {
    value: ethers.parseEther("1000.0"),
  });
  const bazeroAdrressBytes = ethers.zeroPadValue(
    ethers.getBytes(bazero.target),
    32,
  );

  await most.addPair(bazeroAdrressBytes, bazeroAdrressBytes, false);
  const tx = await most.unpause();
  await tx.wait(1);

  await most.sendRequestNative(bazeroAdrressBytes, {
    value: ethers.parseEther("10.0").toString(),
  });

  const _dest = ethers.zeroPadValue(ethers.getBytes(accounts[0]), 32);
  const _request_hash = ethers.keccak256(
    ethers.solidityPacked(
      ["uint256", "bytes32", "uint256", "bytes32", "uint256"],
      [0, bazeroAdrressBytes, 10n * 10n ** 12n, _dest, 0],
    ),
  );

  await most.receiveRequest(
    _request_hash,
    0,
    bazeroAdrressBytes,
    10n * 10n ** 12n,
    _dest,
    0,
  );
}
main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
