import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import MembraneConstructors from '../types/constructors/membrane';
import { uploadCode, Addresses, storeAddresses } from './utils';
import 'dotenv/config';
import '@polkadot/api-augment';

const wsProvider = new WsProvider(process.env.WS_NODE);
const keyring = new Keyring({ type: 'sr25519' });

async function main(): Promise<void> {
    const api = await ApiPromise.create({ provider: wsProvider });
    const deployer = keyring.addFromUri(process.env.AUTHORITY_SEED!);

    const membraneCodeHash = "0x4320907a9497a71c7a5b87b9e395d81eea6d3d6500fa213c7f6400fa9af16922"; //await uploadCode(api, deployer, "membrane.contract");
    console.log('membrane code hash:', membraneCodeHash);

    const membraneConstructors = new MembraneConstructors(api, deployer);

    // const { address: membraneAddress } = await membraneConstructors.new(
    //   [process.env.AUTHORITY!],
    //   process.env.SIGNATURE_THRESHOLD!
    // );

    const membraneAddress = "5CVf2YP2FcjrNhf16uz3tEWH87771QDAGudVaioPLxtWiDh6";

    console.log('membrane address:', membraneAddress);

    // TODO : PSP22 token

    const addresses: Addresses = {
      membraneCodeHash: membraneCodeHash,        
      membraneAddress: membraneAddress
    };

    console.log('addresses:', addresses);
    
    storeAddresses(addresses);

    await api.disconnect();
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
