import fs from "fs";

import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { uploadCode, import_env } from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";

const envFile = process.env.AZERO_ENV;

async function main(): Promise<void> {
  if (!envFile) {
    throw new Error("Please provide an env file");
  }

  const config = await import_env(envFile);

  const { ws_node, deployer_seed } = config;

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);
  console.log("Using", deployer.address, "as the deployer");

  const tokenCodeHash = await uploadCode(api, deployer, "token.contract");
  console.log("token code hash:", tokenCodeHash);

  const wrappedAzeroCodeHash = await uploadCode(
    api,
    deployer,
    "wrapped_azero.contract",
  );
  console.log("wrapped azero code hash:", wrappedAzeroCodeHash);

  const mostL2CodeHash = await uploadCode(api, deployer, "most_l2.contract");
  console.log("most-l2 code hash:", mostL2CodeHash);

  const advisoryCodeHash = await uploadCode(api, deployer, "advisory.contract");
  console.log("advisory code hash:", advisoryCodeHash);

  const code_hashes = {
    most_l2: mostL2CodeHash,
    advisory: advisoryCodeHash,
    token: tokenCodeHash,
  };

  console.log("Current code hashes: ", code_hashes);

  fs.writeFileSync(
    __dirname + "/../l2_code_hashes.json",
    JSON.stringify(code_hashes, null, 2),
  );

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
