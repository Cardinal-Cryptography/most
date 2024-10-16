import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import Most from "../types/contracts/most";
import Token from "../types/contracts/token";
import {
  import_env,
  import_azero_addresses,
  accountIdToHex,
  hexToBytes,
  findTokenBySymbol,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";
import { KeyringPair } from "@polkadot/keyring/types";
import type BN from "bn.js";

// Moved away from utils.ts as L2 scripts do not require it
// and they keep on crashing when the JSON file is non-existent
async function import_eth_addresses() {
  return await import(`../../eth/addresses.json`);
}

const envFile = process.env.AZERO_ENV;

async function addTokenPair(
  tokenEth: string,
  tokenAzero: string,
  is_native_psp22: boolean,
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
    is_native_psp22,
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
  if (!envFile) {
    throw new Error("Please provide an env file");
  }

  const config = await import_env(envFile);

  const { ws_node, deployer_seed, dev } = config;

  const alephAddresses = await import_azero_addresses();
  const ethAddresses = await import_eth_addresses();
  const most_azero = alephAddresses.most;

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);

  console.log("Using ", deployer.address, "as the transaction signer");

  // premint some token for DEV
  if (dev) {
    for (let { address: azero_address } of alephAddresses.ethTokens) {
      await mintTokens(
        azero_address,
        1000000000000000,
        deployer.address,
        deployer,
        api,
        most_azero,
      );
    }
  }

  const most = new Most(most_azero, deployer, api);

  // Add token pairs for Ethereum -> Aleph direction
  for (let {
    symbol: symbol,
    address: aleph_address,
  } of alephAddresses.ethTokens) {
    const { address: eth_address } = findTokenBySymbol(
      symbol,
      ethAddresses.ethTokens,
    );
    await addTokenPair(eth_address, aleph_address, false, most);
  }

  // Add token pairs for Aleph -> Ethereum direction
  for (let {
    symbol: symbol,
    address: aleph_address,
  } of alephAddresses.alephTokens) {
    const { address: eth_address } = findTokenBySymbol(
      symbol,
      ethAddresses.alephTokens,
    );
    await addTokenPair(eth_address, aleph_address, true, most);
  }

  // Set WETH address
  await most.tx.setWeth(
    findTokenBySymbol("WETH", alephAddresses.ethTokens).address,
  );

  // Set WAZERO address
  await most.tx.setWazero(
    findTokenBySymbol("wAZERO", alephAddresses.alephTokens).address,
  );

  if (dev) {
    await most.tx.setHalted(false);
  }

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
