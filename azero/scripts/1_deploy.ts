import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { KeyringPair } from "@polkadot/keyring/types";
import Migrations from "../types/contracts/migrations";
import MigrationsConstructors from "../types/constructors/migrations";
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
import { AccountId } from "../types/types-arguments/token";
import type BN from "bn.js";

const envFile = process.env.AZERO_ENV || "dev";

type TokenArgs = [
  initialSupply: string | number | BN,
  name: string,
  symbol: string,
  decimals: string | number | BN,
  minterBurner: AccountId,
];

async function deployToken(
  tokenArgs: TokenArgs,
  api: ApiPromise,
  deployer: KeyringPair,
) {
  const estimatedGasToken = await estimateContractInit(
    api,
    deployer,
    "token.contract",
    tokenArgs,
  );
  const tokenConstructors = new TokenConstructors(api, deployer);
  return await tokenConstructors.new(...tokenArgs, {
    gasLimit: estimatedGasToken,
  });
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

  const migrationsCodeHash = await uploadCode(
    api,
    deployer,
    "migrations.contract",
  );
  console.log("migrations code hash:", migrationsCodeHash);

  const tokenCodeHash = await uploadCode(api, deployer, "token.contract");
  console.log("token code hash:", tokenCodeHash);

  const mostCodeHash = await uploadCode(api, deployer, "most.contract");
  console.log("most code hash:", mostCodeHash);

  const oracleCodeHash = await uploadCode(api, deployer, "oracle.contract");
  console.log("oracle code hash:", oracleCodeHash);

  const advisoryCodeHash = await uploadCode(api, deployer, "advisory.contract");
  console.log("advisory code hash:", advisoryCodeHash);

  const migrationsConstructors = new MigrationsConstructors(api, deployer);
  const mostConstructors = new MostConstructors(api, deployer);
  const oracleConstructors = new OracleConstructors(api, deployer);
  const advisoryConstructors = new AdvisoryConstructors(api, deployer);

  let estimatedGasMigrations = await estimateContractInit(
    api,
    deployer,
    "migrations.contract",
    [authority],
  );

  const { address: migrationsAddress } = await migrationsConstructors.new(
    authority, // owner
    { gasLimit: estimatedGasMigrations },
  );

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

  const minterBurner =
    process.env.AZERO_ENV == "dev" || process.env.AZERO_ENV == "bridgenet"
      ? authority
      : mostAddress;

  const wethArgs: TokenArgs = [0, "wETH", "Wrapped Ether", 18, minterBurner];

  const usdtArgs: TokenArgs = [0, "USDT", "Tether", 6, minterBurner];

  const { address: wethAddress } = await deployToken(wethArgs, api, deployer);
  console.log("wETH address:", wethAddress);

  const { address: usdtAddress } = await deployToken(usdtArgs, api, deployer);
  console.log("USDT address:", usdtAddress);

  // update migrations
  const migrations = new Migrations(migrationsAddress, deployer, api);
  await migrations.tx.setCompleted(1);

  const addresses: Addresses = {
    migrations: migrationsAddress,
    most: mostAddress,
    weth: wethAddress,
    usdt: usdtAddress,
    oracle: oracleAddress,
    advisory: advisoryAddress,
  };
  console.log("addresses:", addresses);

  storeAddresses(addresses);

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
