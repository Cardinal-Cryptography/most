import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { KeyringPair } from "@polkadot/keyring/types";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import OracleConstructors from "../types/constructors/oracle";
import AdvisoryConstructors from "../types/constructors/advisory";
import {
  uploadCode,
  Addresses,
  storeAddresses,
  estimateContractInit,
  import_env,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { ethers } from "ethers";
import { AccountId } from "../types/types-arguments/token";
import type BN from "bn.js";

const envFile = process.env.AZERO_ENV || "dev";


async function deployToken(
  initialSupply: string | number | BN,
  name: string,
  symbol: string,
  decimals: string | number | BN,
  minterBurner: AccountId,
  api: ApiPromise,
  deployer: KeyringPair
) {
  const estimatedGasToken = await estimateContractInit(
    api,
    deployer,
    "token.contract",
    [initialSupply, name, symbol, decimals, minterBurner]
  );
  const tokenConstructors = new TokenConstructors(api, deployer);
  return await tokenConstructors.new(
    initialSupply,
    name,
    symbol,
    decimals,
    minterBurner,
    { gasLimit: estimatedGasToken }
  );
}

async function main(): Promise<void> {
  const config = await import_env(envFile);

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
  const oracleConstructors = new OracleConstructors(api, deployer);
  const advisoryConstructors = new AdvisoryConstructors(api, deployer);

  let estimatedGasAdvisory = await estimateContractInit(
    api,
    deployer,
    "advisory.contract",
    [authority]
  );

  const { address: advisoryAddress } = await advisoryConstructors.new(
    authority, // owner
    { gasLimit: estimatedGasAdvisory }
  );

  let estimatedGasOracle = await estimateContractInit(
    api,
    deployer,
    "oracle.contract",
    [authority, 10000000000]
  );

  const { address: oracleAddress } = await oracleConstructors.new(
    authority, // owner
    10000000000, // initial value
    { gasLimit: estimatedGasOracle }
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
    ]
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
    { gasLimit: estimatedGasMost }
  );

  console.log("most address:", mostAddress);

  const wethArgs = {
    initialSupply: 0,
    symbol: "wETH",
    name: "wETH",
    decimals: 18,
  };

  const usdtArgs = {
    initialSupply: 0,
    symbol: "USDT",
    name: "USDT",
    decimals: 6,
  };

  const minterBurner =
    process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet"
      ? authority
      : mostAddress;

  const { address: wethAddress } = await deployToken(
    wethArgs.initialSupply,
    wethArgs.name,
    wethArgs.symbol,
    wethArgs.decimals,
    minterBurner,
    api,
    deployer
  );
  console.log("wETH address:", wethAddress);

  const { address: usdtAddress } = await deployToken(
    usdtArgs.initialSupply,
    usdtArgs.name,
    usdtArgs.symbol,
    usdtArgs.decimals,
    minterBurner,
    api,
    deployer
  );
  console.log("USDT address:", usdtAddress);

  const addresses: Addresses = {
    most: mostAddress,
    weth: wethAddress,
    usdt: usdtAddress,
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
