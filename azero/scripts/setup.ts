import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
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

const envFile = process.env.AZERO_ENV || "dev";

async function main(): Promise<void> {
  const config = await import_env(envFile);

  const { ws_node, authority_seed, authority } = config;

  const {
    most: most_azero,
    weth: weth_azero,
    usdt: usdt_azero,
  } = await import_azero_addresses();

  const { weth: weth_eth, usdt: usdt_eth } = await import_eth_addresses();

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(authority_seed);

  // premint some token for DEV
  if (process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet") {
    const weth = new Token(weth_azero, deployer, api);
    await weth.tx.mint(authority, 1000000000000000);
    await weth.tx.setMinterBurner(most_azero);
    const usdt = new Token(usdt_azero, deployer, api);
    await usdt.tx.mint(authority, 1000000000000000);
    await usdt.tx.setMinterBurner(most_azero);
  }

  const most = new Most(most_azero, deployer, api);

  const wethEthAddress = ethers.zeroPadValue(ethers.getBytes(weth_eth), 32);
  console.log("weth eth address:", wethEthAddress);

  const wethHex = accountIdToHex(weth_azero);
  console.log("Adding weth pair to most:", wethHex, wethEthAddress);
  await most.tx.addPair(hexToBytes(wethHex), hexToBytes(wethEthAddress));

  const usdtEthAddress = ethers.zeroPadValue(ethers.getBytes(usdt_eth), 32);
  console.log("usdt eth address:", usdtEthAddress);

  const usdtHex = accountIdToHex(usdt_azero);
  console.log("Adding weth pair to most:", usdtHex, usdtEthAddress);
  await most.tx.addPair(hexToBytes(usdtHex), hexToBytes(usdtEthAddress));

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
