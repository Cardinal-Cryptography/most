let azero = require("../azero/addresses.json")
let eth = require("../eth/addresses.json")

let version = process.env.VERSION;
if (version === undefined) throw new Error("VERSION env var not set");

let result = {
    version: "0.1",
    contract_version: version,
    addresses: {
        azero_governance: {
            contract: "azero_governance",
            address: azero.governance
        },

        azero_most: {
            contract: "azero_most",
            address: azero.most
        },

        azero_weth: {
            contract: "azero_token",
            address: azero.weth
        },

        azero_oracle: {
            contract: "azero_oracle",
            address: azero.oracle
        },

        eth_migrations: {
            contract: "azero_migrations",
            address: azero.migrations
        },

        eth_governance: {
            contract: "eth_governance",
            address: eth.governance
        },

        eth_most: {
            contract: "eth_most",
            address: eth.most
        },

        eth_weth9: {
            contract: "eth_weth9",
            address: eth.weth9
        },

        eth_usdt: {
            contract: "eth_token",
            address: eth.usdt
        },
    }
}

console.log(JSON.stringify(result, null, "  "))
