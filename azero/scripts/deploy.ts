import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import MembraneConstructors from '../types/constructors/membrane';
import TokenConstructors from '../types/constructors/token';
import GovernanceConstructors from '../types/constructors/governance';
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

  const tokenCodeHash = await uploadCode(api, deployer, "token.contract");
  console.log('token code hash:', tokenCodeHash);

  const membraneCodeHash = await uploadCode(api, deployer, "membrane.contract");
  console.log('membrane code hash:', membraneCodeHash);

  const governanceCodeHash = await uploadCode(api, deployer, "governance.contract");
  console.log('governance code hash:', governanceCodeHash);

  const governanceConstructors = new GovernanceConstructors(api, deployer);
  const membraneConstructors = new MembraneConstructors(api, deployer);
  const tokenConstructors = new TokenConstructors(api, deployer);

  const { address: membraneAddress } = await membraneConstructors.new(
    [authority],
    signature_threshold!
  );
  console.log('membrane address:', membraneAddress);

  const { address: wethAddress } = await tokenConstructors.new(
    0, // initial supply
    "wETH", // name
    "wETH", // symbol
    12 // decimals
  );
  console.log('token address:', wethAddress);

  const { address: governanceAddress } = await governanceConstructors.new(
    2 // quorum
  );
  console.log('governance address:', governanceAddress);

  const addresses: Addresses = {
    governance: governanceAddress,
    membrane: membraneAddress,
    weth: wethAddress
  };
  console.log('addresses:', addresses);

  storeAddresses(addresses);

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
