import fs from 'fs';

import { ApiPromise } from '@polkadot/api';
import { Abi } from '@polkadot/api-contract';
import { KeyringPair } from '@polkadot/keyring/types';
import { HexString } from '@polkadot/util/types';
import {
    ContractInstantiateResult,
    WeightV2,
} from '@polkadot/types/interfaces';

export type Addresses = {
  governance: string;
  most: string;
  weth: string;
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
        fs.readFileSync(__dirname + `/../artifacts/` + contractName, 'utf8'),
    );
    const tokenAbi = new Abi(tokenContractRaw);
    const _txHash = await new Promise(async (resolve, reject) => {
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
        __dirname + '/../addresses.json',
        JSON.stringify(addresses, null, 2),
    );
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
    fs.readFileSync(__dirname + `/../artifacts/` + contractName, 'utf8'),
  );
  const contractAbi = new Abi(contractRaw);
  const { gasRequired } = (await api.call.contractsApi.instantiate(
    deployer.address,
    0,
    null,
    null,
    { Upload: contractAbi.info.source.wasm },
    contractAbi.constructors[0].toU8a(sampleArgs),
    '',
  )) as unknown as ContractInstantiateResult;
  return gasRequired;
}
