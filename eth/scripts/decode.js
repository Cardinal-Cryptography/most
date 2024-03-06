const ethers = require("ethers");

const CONTRACT = process.env.CONTRACT;
const CALL = process.env.CALL;
const API =
  process.env["NETWORK"] && process.env["NETWORK"] !== "mainnet"
    ? `https://api-${process.env["NETWORK"]}.etherscan.io/`
    : "https://api.etherscan.io/";

async function main() {
  let response = await fetch(
    `${API}/api?module=contract&action=getsourcecode&address=${CONTRACT}`,
  );
  let source = await response.json();
  if (source.status !== "1") {
    throw new Error(`Failed to fetch contract source code: ${source.result}`);
  }

  let contractName = source.result[0].ContractName;
  let abi = source.result[0].ABI;
  let interface = new ethers.Interface(abi);
  let decoded = interface.parseTransaction({ data: CALL, value: 0 });

  console.log("Contract:", contractName);
  console.log("Method:", decoded.signature);
  console.log("Arguments:", decoded.args);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
