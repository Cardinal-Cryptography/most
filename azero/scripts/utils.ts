import fs from "fs";

import { ApiPromise, Keyring } from "@polkadot/api";
import { Abi } from "@polkadot/api-contract";
import { KeyringPair } from "@polkadot/keyring/types";
import { HexString } from "@polkadot/util/types";
import { hexToU8a, u8aToHex } from "@polkadot/util";
import {
  ContractInstantiateResult,
  WeightV2,
} from "@polkadot/types/interfaces";

export type Addresses = {
  most: string;
  weth: string;
  usdt: string;
  oracle: string;
  advisory: string;
};

/**
 * Uploads the contract to the chain.
 * @param api - The api instance.
 * @param deployer - The deployer keyring pair.
 * @param contractName - The file name of the contract to upload.
 * @returns code hash of the deployed contract.
 */
export async function uploadCode(
  api: ApiPromise,
  deployer: KeyringPair,
  contractName: string,
): Promise<HexString> {
  const tokenContractRaw = JSON.parse(
    fs.readFileSync(__dirname + `/../artifacts/` + contractName, "utf8"),
  );
  const tokenAbi = new Abi(tokenContractRaw);
  await new Promise(async (resolve, reject) => {
    const unsub = await api.tx.contracts
      .uploadCode(tokenAbi.info.source.wasm, null, 0)
      .signAndSend(deployer, (result) => {
        if (result.isFinalized) {
          unsub();
          resolve(result.txHash);
        }
        if (result.isError) {
          unsub();
          reject(result);
        }
      });
  });
  return tokenAbi.info.source.wasmHash.toHex();
}

/**
 * Stores addresses in a JSON file.
 * @param addresses - The addresses to store.
 */
export function storeAddresses(addresses: Addresses): void {
  fs.writeFileSync(
    __dirname + "/../addresses.json",
    JSON.stringify(addresses, null, 2),
  );
}

export async function import_env(envFile: string) {
  return await import(`../env/${envFile}.json`);
}

export async function import_eth_addresses() {
  return await import(`../../eth/addresses.json`);
}

export async function import_azero_addresses() {
  return await import(__dirname + "/../addresses.json");
}

export function hexToBytes(hex: string): number[] {
  let u8array = hexToU8a(hex);
  return Array.from(u8array);
}

export function accountIdToHex(accountId: string): string {
  return u8aToHex(new Keyring({ type: "sr25519" }).decodeAddress(accountId));
}

/**
 * Estimates gas required to create a new instance of the contract.
 *
 * NOTE: This shouldn't be necessary but `Contract::new()` doesn't estimate gas and uses a hardcoded value.
 */
export async function estimateContractInit(
  api: ApiPromise,
  deployer: KeyringPair,
  contractName: string,
  sampleArgs: unknown[],
): Promise<WeightV2> {
  const contractRaw = JSON.parse(
    fs.readFileSync(__dirname + `/../artifacts/` + contractName, "utf8"),
  );
  const contractAbi = new Abi(contractRaw);
  const { gasRequired } = (await api.call.contractsApi.instantiate(
    deployer.address,
    0,
    null,
    null,
    { Upload: contractAbi.info.source.wasm },
    contractAbi.constructors[0].toU8a(sampleArgs),
    "",
  )) as unknown as ContractInstantiateResult;
  return gasRequired;
}

/**
 * Sleeps the thread for a set amount of time
 * @param time - milliseconds
 * NOTE: async function
 */
export async function delay(time) {
  return new Promise((resolve) => setTimeout(resolve, time));
}
