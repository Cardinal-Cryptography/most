import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import MostL2 from "../types/contracts/most_l2";
import Token from "../types/contracts/token";
import {
    import_env,
    import_azero_addresses,
    accountIdToHex,
    hexToBytes,
    import_eth_addresses,
    findTokenBySymbol,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";
import { KeyringPair } from "@polkadot/keyring/types";
import type BN from "bn.js";

const envFile = process.env.AZERO_ENV;

async function addTokenPair(
    tokenEth: string,
    tokenAzero: string,
    mostContract: MostL2,
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

async function bAzeroAddress() {
    const addresses = await import(`../../eth/l2_addresses.json`);
    console.log(addresses);
    return addresses.bazero;
}

async function main(): Promise<void> {
    if (!envFile) {
        throw new Error("Please provide an env file");
    }

    const config = await import_env(envFile);

    const { ws_node, deployer_seed, dev } = config;

    const alephAddresses =  await import(__dirname + "/../l2_addresses.json");
    const most_azero = alephAddresses.mostL2;

    const wsProvider = new WsProvider(ws_node);
    const keyring = new Keyring({ type: "sr25519" });

    const api = await ApiPromise.create({ provider: wsProvider });
    const deployer = keyring.addFromUri(deployer_seed);

    console.log("Using ", deployer.address, "as the transaction signer");

    const most = new MostL2(most_azero, deployer, api);

    const bazeroL2Address = await bAzeroAddress();
    const wAzeroAddress = findTokenBySymbol("wAZERO", alephAddresses.alephTokens).address;
    console.log(bazeroL2Address, wAzeroAddress);

    // Set WAZERO address
    await most.tx.setWazero(
        wAzeroAddress
    );
    await addTokenPair(bazeroL2Address, wAzeroAddress, most);


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
