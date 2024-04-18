import fs from "fs";

import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import {
  uploadCode,
  import_env,
  import_azero_addresses,
  hexToBytes,
} from "./utils";
import Migrations from "../types/contracts/migrations";
import Most from "../types/contracts/most";
import "dotenv/config";
import "@polkadot/api-augment";

const envFile = process.env.AZERO_ENV || "dev";

async function main(): Promise<void> {
  const config = await import_env(envFile);

  const { ws_node, deployer_seed, dev } = config;

  const { most: most_azero, migrations: migrations_azero } =
    await import_azero_addresses();

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });

  const api = await ApiPromise.create({ provider: wsProvider });
  const deployer = keyring.addFromUri(deployer_seed);
  console.log("Using", deployer.address, "as the deployer");

  const migrations = new Migrations(migrations_azero, deployer, api);

  // check migrations
  let lastCompletedMigration = await migrations.query.lastCompletedMigration();
  const number = lastCompletedMigration.value.ok;
  console.log("Last completed migration: ", number);
  if (number != 2) {
    console.error("Previous migration has not been completed");
    process.exit(-1);
  }

  const mostCodeHash = await uploadCode(api, deployer, "most.contract");
  console.log("most code hash:", mostCodeHash);

  if (dev) {
    // in dev environment where ALice is the owner we can just set code hash
    const most = new Most(most_azero, deployer, api);
    await most.tx.setCode(hexToBytes(mostCodeHash));
  } else {
    const code_hash = {
      most: mostCodeHash,
    };

    fs.writeFileSync(
      __dirname + "/../upgraded_code_hash.json",
      JSON.stringify(code_hash, null, 2),
    );

    console.log(
      "Guardians need to vote for am upgrade Most code hash: ",
      code_hash,
    );
  }

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
