import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import Migrations from "../types/contracts/migrations";
import Most from "../types/contracts/most";
import Token from "../types/contracts/token";
import {
  import_env,
  import_azero_addresses,
  import_eth_addresses,
  accountIdToHex,
  hexToBytes,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";
import { KeyringPair } from "@polkadot/keyring/types";
import type BN from "bn.js";

const envFile = process.env.AZERO_ENV || "dev";

async function addTokenPair(
  tokenEth: string,
  tokenAzero: string,
  mostContract: Most,
) {
  const tokenEthAddress = ethers.zeroPadValue(ethers.getBytes(tokenEth), 32);
  const tokenAzeroAddress = accountIdToHex(tokenAzero);
  console.log(
    `Adding token pair to Most: ${tokenAzeroAddress} => ${tokenEthAddress}`,
  );
  await mostContract.tx.addPair(
    hexToBytes(tokenAzeroAddress),
    hexToBytes(tokenEthAddress),
  );
}

async function mintTokens(
  tokenAddress: string,
  amount: number | BN | string,
  to: string,
  signer: KeyringPair,
  api: ApiPromise,
  mostAddress?: string,
) {
  const weth = new Token(tokenAddress, signer, api);
  await weth.tx.mint(to, amount);
  if (mostAddress) {
    await weth.tx.setMinterBurner(mostAddress);
  }
}

async function main(): Promise<void> {
  const config = await import_env(envFile);

  const { ws_node, deployer_seed } = config;

  const {
    most: most_azero,
    weth: weth_azero,
    usdt: usdt_azero,
    migrations: migrations_azero,
  } = await import_azero_addresses();

  const { weth: weth_eth, usdt: usdt_eth } = await import_eth_addresses();

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);

  const migrations = new Migrations(migrations_azero, deployer, api);

  // check migrations
  let lastCompletedMigration = await migrations.query.lastCompletedMigration();
  const number = lastCompletedMigration.value.ok;
  console.log("Last completed migration: ", number);
  if (number != 1) {
    console.error("Previous migration has not been completed");
    process.exit(-1);
  }

  // premint some token for DEV
  if (process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet") {
    await mintTokens(
      weth_azero,
      1000000000000000,
      deployer.address,
      deployer,
      api,
      most_azero,
    );
    await mintTokens(
      usdt_azero,
      1000000000000000,
      deployer.address,
      deployer,
      api,
      most_azero,
    );

    const most = new Most(most_azero, deployer, api);

    await addTokenPair(weth_eth, weth_azero, most);
    await addTokenPair(usdt_eth, usdt_azero, most);
  }

  // update migrations
  await migrations.tx.setCompleted(2);

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
