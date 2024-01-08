let azero = require("../azero/addresses.json")
let eth = require("../eth/addresses.json")

let version = process.env.VERSION;
if (version === undefined) throw new Error("VERSION env var not set");

let result = {
    version: "0.1",
    addresses: {
        azero_governance: {
            version,
            contract: "most_azero/governance",
            address: azero.governance
        },

        azero_most: {
            version,
            contract: "most_azero/most",
            address: azero.most
        },

        azero_weth: {
            version,
            contract: "most_azero/weth",
            address: azero.weth
        },

        eth_governance: {
            version,
            contract: "most_eth/governance",
            address: eth.governance
        },

        eth_most: {
            version,
            contract: "most_eth/most",
            address: eth.most
        },

        eth_weth9: {
            version,
            contract: "most_eth/weth9",
            address: eth.weth9
        },

        eth_usdt: {
            version,
            contract: "most_eth/usdt",
            address: eth.usdt
        },
    }
}

console.log(result)