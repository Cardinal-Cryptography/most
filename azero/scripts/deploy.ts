import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import GovernanceConstructors from "../types/constructors/governance";
import {
  uploadCode,
  Addresses,
  storeAddresses,
  estimateContractInit,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";

const envFile = process.env.AZERO_ENV || "dev";
async function import_env() {
  return await import(`../env/${envFile}.json`);
}

async function main(): Promise<void> {
  const {
    ws_node,
    relayers_keys,
    authority_seed,
    signature_threshold,
    commission_per_dix_mille,
    pocket_money,
    minimum_transfer_amount_usd,
    relay_gas_usage,
  } = await import_env();

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(authority_seed);

  const tokenCodeHash = await uploadCode(api, deployer, "token.contract");
  console.log("token code hash:", tokenCodeHash);

  const mostCodeHash = await uploadCode(api, deployer, "most.contract");
  console.log("most code hash:", mostCodeHash);

  const governanceCodeHash = await uploadCode(
    api,
    deployer,
    "governance.contract",
  );
  console.log("governance code hash:", governanceCodeHash);

  const governanceConstructors = new GovernanceConstructors(api, deployer);
  const mostConstructors = new MostConstructors(api, deployer);
  const tokenConstructors = new TokenConstructors(api, deployer);

  const estimatedGasMost = await estimateContractInit(
    api,
    deployer,
    "most.contract",
    [
      relayers_keys,
      signature_threshold!,
      commission_per_dix_mille!,
      pocket_money!,
      minimum_transfer_amount_usd!,
      relay_gas_usage!,
    ],
  );

  const { address: mostAddress } = await mostConstructors.new(
    relayers_keys,
    signature_threshold!,
    commission_per_dix_mille!,
    pocket_money!,
    minimum_transfer_amount_usd!,
    relay_gas_usage!,
    { gasLimit: estimatedGasMost },
  );

  console.log("most address:", mostAddress);

  const initialSupply = 0;
  const symbol = "wETH";
  const name = symbol;
  const decimals = 12;
  const minterBurner = mostAddress
  const estimatedGasToken = await estimateContractInit(
    api,
    deployer,
    "token.contract",
    [initialSupply, name, symbol, decimals, minterBurner],
  );

  const { address: wethAddress } = await tokenConstructors.new(
    initialSupply, name, symbol, decimals, minterBurner,
    { gasLimit: estimatedGasToken },
  );
  console.log("token address:", wethAddress);

  const quorum = 2;
  const estimatedGasGovernance = await estimateContractInit(
    api,
    deployer,
    "governance.contract",
    [quorum],
  );

  const governance = await governanceConstructors.new(
    quorum,
    { gasLimit: estimatedGasGovernance },
  );
  const governanceAddress = governance.address;
  console.log("governance address:", governanceAddress);

  const addresses: Addresses = {
    governance: governanceAddress,
    most: mostAddress,
    weth: wethAddress,
  };
  console.log("addresses:", addresses);

  storeAddresses(addresses);

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
