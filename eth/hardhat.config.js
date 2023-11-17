require("@nomicfoundation/hardhat-ethers");
require("@nomicfoundation/hardhat-chai-matchers");

// The default account generated from this mnemonic is Ee88da44b4901d7F86970c52dC5139Af80C83edD.
// This account is pre-seeded with money locally and on bridgenet.
const DEV_MNEMONIC =
  "harsh master island dirt equip search awesome double turn crush wool grant";

module.exports = {
    defaultNetwork: "hardhat",
    networks: {
      hardhat: {
      },
      development: {
        url: "http://127.0.0.1:8545",
        accounts: {
            mnemonic: DEV_MNEMONIC,
        },
        gas: 25e6, // Gas limit
        gasPrice: 20e9,
      },
      bridgenet: {
        url: "https://rpc-eth-node2-bridgenet.dev.azero.dev",
        accounts: {
            mnemonic: DEV_MNEMONIC,
        },
        chainId: 12345,
        gas: 25e6, // Gas limit
        gasPrice: 20e9,
      }
    },
    solidity: {
        compilers: [
            {
                version: "0.8.0",
                settings: {
                    optimizer: {
                        enabled: true,
                        runs: 200
                    }
                }
            },
            {
                version: "0.4.18",
                settings: {
                    optimizer: {
                        enabled: true,
                        runs: 200
                    }
                }
            },
        ]
    },
    paths: {
      sources: "./contracts",
      tests: "./test",
      cache: "./cache",
      artifacts: "./artifacts"
    },
    mocha: {
      timeout: 40000
    }
  }
