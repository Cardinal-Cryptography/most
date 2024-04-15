import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { KeyringPair } from "@polkadot/keyring/types";
import Migrations from "../types/contracts/migrations";
import MigrationsConstructors from "../types/constructors/migrations";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import OracleConstructors from "../types/constructors/oracle";
import AdvisoryConstructors from "../types/constructors/advisory";
import {
  estimateContractInit,
  import_env,
  storeAddresses,
  Addresses,
  import_eth_addresses,
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
  const ethAddresses = await import_eth_addresses();

  const {
    ws_node,
    relayers,
    deployer_seed,
    signature_threshold,
    pocket_money,
    relay_gas_usage,
    min_gas_price,
    max_gas_price,
    default_gas_price,
    gas_oracle_max_age,
    oracle_call_gas_limit,
    base_fee_buffer_percentage,
    tokens,
    dev,
  } = config;

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);
  console.log("Using", deployer.address, "as the deployer");

  const migrationsConstructors = new MigrationsConstructors(api, deployer);
  const mostConstructors = new MostConstructors(api, deployer);
  const oracleConstructors = new OracleConstructors(api, deployer);
  const advisoryConstructors = new AdvisoryConstructors(api, deployer);

  let estimatedGasMigrations = await estimateContractInit(
    api,
    deployer,
    "migrations.contract",
    [deployer.address],
  );

  const { address: migrationsAddress } = await migrationsConstructors.new(
    deployer.address, // owner
    { gasLimit: estimatedGasMigrations },
  );

  let estimatedGasAdvisory = await estimateContractInit(
    api,
    deployer,
    "advisory.contract",
    [deployer.address],
  );

  const { address: advisoryAddress } = await advisoryConstructors.new(
    deployer.address, // owner
    { gasLimit: estimatedGasAdvisory },
  );

  let estimatedGasOracle = await estimateContractInit(
    api,
    deployer,
    "oracle.contract",
    [deployer.address, 10000000000],
  );

  const { address: oracleAddress } = await oracleConstructors.new(
    deployer.address, // owner
    10000000000, // initial value
    { gasLimit: estimatedGasOracle },
  );

  const estimatedGasMost = await estimateContractInit(
    api,
    deployer,
    "most.contract",
    [
      relayers,
      signature_threshold!,
      pocket_money!,
      relay_gas_usage!,
      min_gas_price!,
      max_gas_price!,
      default_gas_price!,
      gas_oracle_max_age!,
      oracle_call_gas_limit!,
      base_fee_buffer_percentage!,
      oracleAddress,
      deployer.address,
    ],
  );

  const { address: mostAddress } = await mostConstructors.new(
    relayers,
    signature_threshold!,
    pocket_money!,
    relay_gas_usage!,
    min_gas_price!,
    max_gas_price!,
    default_gas_price!,
    gas_oracle_max_age!,
    oracle_call_gas_limit!,
    base_fee_buffer_percentage!,
    oracleAddress,
    deployer.address,
    { gasLimit: estimatedGasMost },
  );

  console.log("most address:", mostAddress);

  const minterBurner = dev ? deployer.address : mostAddress;
  var tokenAddresses = [];
  for (let token of tokens) {
    const initialSupply = 0;
    const tokenArgs: TokenArgs = [
      initialSupply,
      token.name,
      token.symbol,
      token.decimals,
      minterBurner,
    ];

    const { address } = await deployToken(tokenArgs, api, deployer);
    console.log(token.symbol, "address:", address);

    let ethAddress = token.checkAddress
      ? ethAddresses[token.checkAddress]
      : token.ethAddress!;

    tokenAddresses.push([token.symbol, ethAddress, address]);
  }

  const migrations = new Migrations(migrationsAddress, deployer, api);
  await migrations.tx.setCompleted(1);

  const addresses: Addresses = {
    migrations: migrationsAddress,
    most: mostAddress,
    oracle: oracleAddress,
    advisory: advisoryAddress,
    tokens: tokenAddresses,
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
