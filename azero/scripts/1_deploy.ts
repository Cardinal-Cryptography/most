import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { KeyringPair } from "@polkadot/keyring/types";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import OracleConstructors from "../types/constructors/oracle";
import AdvisoryConstructors from "../types/constructors/advisory";
import WrappedAzeroConstructors from "../types/constructors/wrapped_azero";
import {
  estimateContractInit,
  import_env,
  storeAddresses,
  Addresses,
  import_token_config,
  findTokenBySymbol,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { AccountId } from "../types/types-arguments/token";
import type BN from "bn.js";

const envFile = process.env.AZERO_ENV;

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
  if (!envFile) {
    throw new Error("Please provide an env file");
  }

  const config = await import_env(envFile);
  const tokenConfigPath = config.token_config_path;
  const tokenConfig = await import_token_config(tokenConfigPath);

  const isWrappedAzeroDeployed = findTokenBySymbol(
    "wAZERO",
    tokenConfig.aleph,
  ).deployed;

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
    eth_transfer_gas_usage,
    dev,
  } = config;

  const tokensEth = tokenConfig.eth;
  const tokensAleph = tokenConfig.aleph;

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);
  console.log("Using", deployer.address, "as the deployer");

  const wrappedAzeroConstructors = new WrappedAzeroConstructors(api, deployer);
  const mostConstructors = new MostConstructors(api, deployer);
  const oracleConstructors = new OracleConstructors(api, deployer);
  const advisoryConstructors = new AdvisoryConstructors(api, deployer);

  var wrappedAzeroAddress = "";
  if (!isWrappedAzeroDeployed) {
    let estimatedGasWrappedAzero = await estimateContractInit(
      api,
      deployer,
      "wrapped_azero.contract",
      [],
    );

    wrappedAzeroAddress = await wrappedAzeroConstructors
      .new({
        gasLimit: estimatedGasWrappedAzero,
      })
      .then((res) => res.address);
  } else {
    wrappedAzeroAddress = findTokenBySymbol("wAZERO", tokensAleph).address;
  }

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
      eth_transfer_gas_usage,
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
    eth_transfer_gas_usage,
    { gasLimit: estimatedGasMost },
  );

  console.log("most address:", mostAddress);

  const minterBurner = dev ? deployer.address : mostAddress;
  var ethTokenAddresses = [];
  for (let token of tokensEth) {
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

    ethTokenAddresses.push({ symbol: token.symbol, address: address });
  }

  var alephTokenAddresses = [];
  for (let token of tokensAleph) {
    if (token.deployed) {
      alephTokenAddresses.push({
        symbol: token.symbol,
        address: token.address,
      });
    }
  }

  if (!isWrappedAzeroDeployed) {
    alephTokenAddresses.push({
      symbol: "wAZERO",
      address: wrappedAzeroAddress,
    });
  }

  const addresses: Addresses = {
    most: mostAddress,
    oracle: oracleAddress,
    advisory: advisoryAddress,
    ethTokens: ethTokenAddresses,
    alephTokens: alephTokenAddresses,
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
