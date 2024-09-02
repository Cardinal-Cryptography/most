import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import MostL2 from "../types/contracts/most_l2";
import { import_env, findTokenBySymbol } from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";
import { KeyringPair } from "@polkadot/keyring/types";
import type BN from "bn.js";
import WrappedAzero from "../types/contracts/wrapped_azero";

const envFile = process.env.AZERO_ENV;

async function main(): Promise<void> {
  if (!envFile) {
    throw new Error("Please provide an env file");
  }

  const config = await import_env(envFile);

  const { ws_node, deployer_seed, azero_deposit } = config;

  const alephAddresses = await import(__dirname + "/../l2_addresses.json");
  const most_azero = alephAddresses.mostL2;

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);

  console.log("Using ", deployer.address, "as the transaction signer");

  const most = new MostL2(most_azero, deployer, api);
  const wAzeroAddress = findTokenBySymbol(
    "wAZERO",
    alephAddresses.alephTokens,
  ).address;

  // Set WAZERO address
  console.log(`wAZERO address set to ${wAzeroAddress}`);
  await most.tx.setWazero(wAzeroAddress);

  console.log("Creating WrappedAzero instance");
  const wAzero = new WrappedAzero(wAzeroAddress, deployer, api);
  const deposit = BigInt(azero_deposit * 10 ** 12);

  console.log("Depositing into wAZERO", deposit);
  await wAzero.tx.deposit({ value: deposit });

  console.log("Transfering wAZERO into MOST", deposit);
  await wAzero.tx.transfer(most_azero, deposit.toString(), []);

  if (config.dev) {
    console.log("Unpausing Most");
    await most.tx.setHalted(false);
  }

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
