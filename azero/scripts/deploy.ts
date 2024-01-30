import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import MostConstructors from "../types/constructors/most";
import TokenConstructors from "../types/constructors/token";
import GovernanceConstructors from "../types/constructors/governance";
import Governance from "../types/contracts/governance";
import Most from "../types/contracts/most";
import Token from "../types/contracts/token";
import OracleConstructors from "../types/constructors/oracle";
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
    governance_keys,
    authority_seed,
    signature_threshold,
    pocket_money,
    relay_gas_usage,
    min_fee,
    max_fee,
    default_fee,
    authority
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

  const oracleCodeHash = await uploadCode(
    api,
    deployer,
    "oracle.contract",
  );
  console.log("oracle code hash:", oracleCodeHash);

  const governanceConstructors = new GovernanceConstructors(api, deployer);
  const mostConstructors = new MostConstructors(api, deployer);
  const tokenConstructors = new TokenConstructors(api, deployer);
  const oracleConstructors = new OracleConstructors(api, deployer);

  let estimatedGasOracle = await estimateContractInit(
    api,
    deployer,
    "oracle.contract",
    [authority, 10000000000],
  );

  const { address: oracleAddress } = await oracleConstructors.new(
    authority, // owner
    10000000000,  // initial value
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
  const decimals = 12;
  const minterBurner = mostAddress;
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

  const quorum = 2;
  const estimatedGasGovernance = await estimateContractInit(
    api,
    deployer,
    "governance.contract",
    [quorum],
  );

  const { address: governanceAddress } = await governanceConstructors.new(
    quorum,
    { gasLimit: estimatedGasGovernance },
  );
  const governance = new Governance(governanceAddress, deployer, api);
  console.log("governance address:", governanceAddress);

  for (const address of governance_keys) {
    console.log("Adding", address, "as governance member...");
    await governance.tx.addMember(address);
  }

  console.log("Transferring ownership of most to governance...");
  await new Most(mostAddress, deployer, api).tx.setOwner(governanceAddress);

  console.log("Transferring ownership of weth to governance...");
  await new Token(wethAddress, deployer, api).tx.setAdmin(governanceAddress);

  console.log("Transferring ownership of governance to governance...");
  await governance.tx.setOwner(governanceAddress);

  const addresses: Addresses = {
    governance: governanceAddress,
    most: mostAddress,
    weth: wethAddress,
    oracle: oracleAddress,
  };
  console.log("addresses:", addresses);

  storeAddresses(addresses);

  await api.disconnect();
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
