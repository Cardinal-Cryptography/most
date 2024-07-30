import fs from "fs";

import { ApiPromise, WsProvider } from "@polkadot/api";
import "dotenv/config";
import "@polkadot/api-augment";

const envFile = process.env.AZERO_ENV;

async function main(): Promise<void> {
  if (!envFile) {
    throw new Error("Please provide an env file");
  }

  const config = await import(`../env/${envFile}.json`);
  const { ws_node } = config;
  const wsProvider = new WsProvider(ws_node);
  const api = await ApiPromise.create({ provider: wsProvider });

  const latestBlockHash = await api.rpc.chain.getFinalizedHead();
  const latestBlock = await api.rpc.chain.getBlock(latestBlockHash);
  const block_latest = {
    number: latestBlock.block.header.number,
  };
  fs.writeFileSync("block_latest.json", JSON.stringify(block_latest));
  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
