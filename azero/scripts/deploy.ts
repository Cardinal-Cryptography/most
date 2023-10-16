import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import MembraneConstructors from '../types/constructors/membrane';
import { uploadCode, Addresses, storeAddresses } from './utils';
import 'dotenv/config';
import '@polkadot/api-augment';

const envFile = process.env.AZERO_ENV || 'dev';
async function import_env() {
  return await import(`../env/${envFile}.json`);
}

async function main(): Promise<void> {
  let {
    ws_node,
    authority,
    authority_seed,
    signature_threshold
  } = await import_env();

  let wsProvider = new WsProvider(ws_node);
  let keyring = new Keyring({ type: 'sr25519' });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(authority_seed);

  const membraneCodeHash = await uploadCode(api, deployer, "membrane.contract");
  console.log('membrane code hash:', membraneCodeHash);

  const membraneConstructors = new MembraneConstructors(api, deployer);

  const { address: membraneAddress } = await membraneConstructors.new(
    [authority],
    signature_threshold!
  );

  console.log('membrane address:', membraneAddress);

  // TODO : deploy PSP22 token

  const addresses: Addresses = {
    membraneCodeHash: membraneCodeHash,
    membrane: membraneAddress
  };

  console.log('addresses:', addresses);

  storeAddresses(addresses);

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
