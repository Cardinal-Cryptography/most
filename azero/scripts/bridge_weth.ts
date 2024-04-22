import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import Most from "../types/contracts/most";
import Token from "../types/contracts/token";
import {
  import_env,
  import_azero_addresses,
  accountIdToHex,
  hexToBytes,
} from "./utils";
import { ethers } from "ethers";

const envFile = process.env.AZERO_ENV;

function getTokenAddresses(addresses, tokenName): [string, string] {
  for (let [_, ethAddress, azeroAddress] of addresses.tokens) {
    if (tokenName == tokenName) {
      return [ethAddress, azeroAddress];
    }
  }
}

// bridges (redeems) 1 unit of wETH from AlephZero to Ethereum
async function main(): Promise<void> {
  const config = await import_env(envFile);
  const { ws_node, deployer_seed } = config;

  const addresses = await import_azero_addresses();
  const weth_azero = getTokenAddresses(addresses, "weth")[1];

  const { most: most_azero } = await import_azero_addresses();

  const wsProvider = new WsProvider(ws_node);
  const keyring = new Keyring({ type: "sr25519" });
  const api = await ApiPromise.create({ provider: wsProvider });

  const signer = keyring.addFromUri(deployer_seed);

  const amount = 1000000000000;
  const receiver = "0xEe88da44b4901d7F86970c52dC5139Af80C83edD";

  const weth = new Token(weth_azero, signer, api);

  // check senders balance
  let response = await weth.query.balanceOf(signer.address);
  let balance = response.value.ok.rawNumber.toNumber();

  console.log(
    "wETH [",
    weth_azero,
    "]",
    "balance of:",
    signer.address,
    ":",
    balance,
  );
  if (balance < amount) {
    // nothing can be done
    throw new Error("Balance too low");
  }

  // TODO check approval

  // approve Most to withdraw
  await weth.tx.approve(most_azero, amount);

  // send request
  const most = new Most(most_azero, signer, api);

  let srcTokenAddress = hexToBytes(accountIdToHex(weth_azero));

  let destReceiverAddress = hexToBytes(
    ethers.zeroPadValue(ethers.getBytes(receiver), 32),
  );

  const fee = 1000000000000000;

  console.log(
    "Requesting transfer of",
    amount,
    "units of",
    weth_azero,
    "[",
    accountIdToHex(weth_azero),
    "]",
    "to",
    receiver,
    "[",
    ethers.zeroPadValue(ethers.getBytes(receiver), 32),
    "]",
  );

  let tx = await most.tx.sendRequest(
    srcTokenAddress,
    amount,
    destReceiverAddress,
    { value: fee },
  );

  console.log(
    "Tx hash:",
    tx.txHash,
    ", block number:",
    tx.result.blockNumber.toNumber(),
  );

  await api.disconnect();
  console.log("Done");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
