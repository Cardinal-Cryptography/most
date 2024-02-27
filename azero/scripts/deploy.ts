import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { hexToU8a, u8aToHex } from "@polkadot/util";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import Most from "../types/contracts/most";
import Token from "../types/contracts/token";
import OracleConstructors from "../types/constructors/oracle";
import AdvisoryConstructors from "../types/constructors/advisory";
import {
  uploadCode,
  Addresses,
  storeAddresses,
  estimateContractInit,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";

const envFile = process.env.AZERO_ENV || "dev";
async function import_env() {
  return await import(`../env/${envFile}.json`);
}

async function import_eth_addresses() {
  return await import(`../../eth/addresses.json`);
}

function hexToBytes(hex: string): number[] {
  let u8array = hexToU8a(hex);
  return Array.from(u8array);
}

function accountIdToHex(accountId: string): string {
  return u8aToHex(new Keyring({ type: "sr25519" }).decodeAddress(accountId));
}

async function main(): Promise<void> {
  const config = await import_env();

  const {
    ws_node,
    relayers_keys,
    authority_seed,
    signature_threshold,
    pocket_money,
    relay_gas_usage,
    min_fee,
    max_fee,
    default_fee,
    authority,
  } = config;

  const { weth } = await import_eth_addresses();
  const wethEthAddress = ethers.zeroPadValue(ethers.getBytes(weth), 32);
  console.log("weth eth address:", wethEthAddress);

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(authority_seed);

  const tokenCodeHash = await uploadCode(api, deployer, "token.contract");
  console.log("token code hash:", tokenCodeHash);

  const mostCodeHash = await uploadCode(api, deployer, "most.contract");
  console.log("most code hash:", mostCodeHash);

  const oracleCodeHash = await uploadCode(api, deployer, "oracle.contract");
  console.log("oracle code hash:", oracleCodeHash);

  const advisoryCodeHash = await uploadCode(api, deployer, "advisory.contract");
  console.log("advisory code hash:", advisoryCodeHash);

  const mostConstructors = new MostConstructors(api, deployer);
  const tokenConstructors = new TokenConstructors(api, deployer);
  const oracleConstructors = new OracleConstructors(api, deployer);
  const advisoryConstructors = new AdvisoryConstructors(api, deployer);

  let estimatedGasAdvisory = await estimateContractInit(
    api,
    deployer,
    "advisory.contract",
    [authority],
  );

  const { address: advisoryAddress } = await advisoryConstructors.new(
    authority, // owner
    { gasLimit: estimatedGasAdvisory },
  );

  let estimatedGasOracle = await estimateContractInit(
    api,
    deployer,
    "oracle.contract",
    [authority, 10000000000],
  );

  const { address: oracleAddress } = await oracleConstructors.new(
    authority, // owner
    10000000000, // initial value
    { gasLimit: estimatedGasOracle },
  );

  const estimatedGasMost = await estimateContractInit(
    api,
    deployer,
    "most.contract",
    [
      relayers_keys,
      signature_threshold!,
      pocket_money!,
      relay_gas_usage!,
      min_fee!,
      max_fee!,
      default_fee!,
      oracleAddress,
    ],
  );

  const { address: mostAddress } = await mostConstructors.new(
    relayers_keys,
    signature_threshold!,
    pocket_money!,
    relay_gas_usage!,
    min_fee!,
    max_fee!,
    default_fee!,
    oracleAddress,
    { gasLimit: estimatedGasMost },
  );

  console.log("most address:", mostAddress);

  const initialSupply = 0;
  const symbol = "wETH";
  const name = symbol;
  const decimals = 18;
  const minterBurner =
    process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet"
      ? authority
      : mostAddress;
  const estimatedGasToken = await estimateContractInit(
    api,
    deployer,
    "token.contract",
    [initialSupply, name, symbol, decimals, minterBurner],
  );

  const { address: wethAddress } = await tokenConstructors.new(
    initialSupply,
    name,
    symbol,
    decimals,
    minterBurner,
    { gasLimit: estimatedGasToken },
  );
  console.log("token address:", wethAddress);

  // premint some token for DEV
  if (process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet") {
    const weth = new Token(wethAddress, deployer, api);
    await weth.tx.mint(authority, 1000000000000000);
    await weth.tx.setMinterBurner(mostAddress);
  }

  const most = new Most(mostAddress, deployer, api);

  const wethHex = accountIdToHex(wethAddress);
  console.log("Adding weth pair to most:", wethHex, wethEthAddress);
  await most.tx.addPair(hexToBytes(wethHex), hexToBytes(wethEthAddress));

  const addresses: Addresses = {
    most: mostAddress,
    weth: wethAddress,
    oracle: oracleAddress,
    advisory: advisoryAddress,
  };
  console.log("addresses:", addresses);

  storeAddresses(addresses);

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
