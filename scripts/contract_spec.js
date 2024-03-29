let azero = require("../azero/addresses.json")
let eth = require("../eth/addresses.json")

let version = process.env.VERSION;
if (version === undefined) throw new Error("VERSION env var not set");

let result = {
    version: "0.1",
    contract_version: version,
    addresses: {

        azero_most: {
            contract: "azero_most",
            address: azero.most
        },

        azero_weth: {
            contract: "azero_token",
            address: azero.weth
        },

        azero_usdt: {
            contract: "azero_token",
            address: azero.usdt
        },

        azero_oracle: {
            contract: "azero_oracle",
            address: azero.oracle
        },

        azero_advisory: {
            contract: "azero_advisory",
            address: azero.advisory
        },

        eth_migrations: {
            contract: "azero_migrations",
            address: eth.migrations
        },

        eth_governance: {
            contract: "eth_governance",
            address: eth.governance
        },

        eth_most: {
            contract: "eth_most",
            address: eth.most
        },

        eth_weth: {
            contract: "eth_weth",
            address: eth.weth
        },

        eth_usdt: {
            contract: "eth_token",
            address: eth.usdt
        },
    }
}

console.log(JSON.stringify(result, null, "  "))
