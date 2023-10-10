import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
// import Factory_factory from '../types/constructors/factory_contract';
// import Wnative_factory from '../types/constructors/wnative_contract';
// import Router_factory from '../types/constructors/router_contract';
// import Factory from '../types/contracts/factory_contract';
// import Wnative from '../types/contracts/wnative_contract';
// import { storeAddresses } from './utils';
// import { Addresses } from './shared';
// import * as pair from './pair';
// import * as factoryUtils from './factory';
// import * as wnativeUtils from './wnative';
// import * as routerUtils from './router';
import 'dotenv/config';
import '@polkadot/api-augment';

// Create a new instance of contract
const wsProvider = new WsProvider(process.env.WS_NODE);
// Create a keyring instance
const keyring = new Keyring({ type: 'sr25519' });

async function main(): Promise<void> {
  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(process.env.AUTHORITY_SEED!);

  // const pairHash = await pair.upload(api, deployer);
  // console.log('pair code hash:', pairHash);

  // const factoryInitGas = await factoryUtils.estimateInit(api, deployer);
  // const factoryFactory = new Factory_factory(api, deployer);
  // const { address: factoryAddress } = await factoryFactory.new(
  //   deployer.address,
  //   pairHash,
  //   { gasLimit: factoryInitGas },
  // );
  // const factory = new Factory(factoryAddress, deployer, api);

  // console.log('factory address:', factoryAddress);

  // const wnativeInitGas = await wnativeUtils.estimateInit(api, deployer);
  // const wnativeFactory = new Wnative_factory(api, deployer);
  // const { address: wnativeAddress } = await wnativeFactory.new({
  //   gasLimit: wnativeInitGas,
  // });
  // const wnative = new Wnative(wnativeAddress, deployer, api);

  // console.log('wnative address:', wnativeAddress);

  // const routerInitGas = await routerUtils.estimateInit(api, deployer);
  // const routerFactory = new Router_factory(api, deployer);
  // const { address: routerAddress } = await routerFactory.new(
  //   factory.address,
  //   wnative.address,
  //   { gasLimit: routerInitGas },
  // );

  // console.log('router address:', routerAddress);

  // const addresses: Addresses = {
  //   pairCodeHash: pairHash,
  //   factoryAddress: factoryAddress,
  //   wnativeAddress: wnativeAddress,
  //   routerAddress: routerAddress,
  // };

  // storeAddresses(addresses);
  // console.log('addresses stored');
  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
