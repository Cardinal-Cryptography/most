require("@nomicfoundation/hardhat-toolbox");
require("@openzeppelin/hardhat-upgrades");
require("@nomicfoundation/hardhat-chai-matchers");
require("@nomiclabs/hardhat-truffle5");
require("@nomicfoundation/hardhat-verify");

// The default account generated from this mnemonic is Ee88da44b4901d7F86970c52dC5139Af80C83edD.
// This account is pre-seeded with money locally and on bridgenet.
const DEV_MNEMONIC =
  "harsh master island dirt equip search awesome double turn crush wool grant";

const ETHERSCAN_API_KEY = process.env.ETHERSCAN_API_KEY;
const SEPOLIA_MNEMONIC = process.env.SEPOLIA_MNEMONIC;
const SEPOLIA_PRIVATE_KEY = process.env.SEPOLIA_PRIVATE_KEY;
const SEPOLIA_ACCOUNT_NUMBER = process.env.SEPOLIA_ACCOUNT_NUMBER;
const SEPOLIA_WETH = process.env.SEPOLIA_WETH;
const SEPOLIA_TOKEN_CONFIG_PATH = process.env.SEPOLIA_TOKEN_CONFIG_PATH;
const SEPOLIA_URL = process.env.SEPOLIA_URL;
const SEPOLIA_THRESHOLD = process.env.SEPOLIA_THRESHOLD;
const ETHEREUM_PRIVATE_KEY = process.env.ETHEREUM_PRIVATE_KEY;
const ETHEREUM_GUARDIAN_ADDRESS = process.env.ETHEREUM_GUARDIAN_ADDRESS;
const EVM_TESTNET_KEY = process.env.EVM_TESTNET_KEY;
const EVM_TESTNET_ACCOUNT_NUMBER = process.env.EVM_TESTNET_ACCOUNT_NUMBER;
const EVM_MAINNET_KEY = process.env.EVM_MAINNET_KEY;
const EVM_MAINNET_ACCOUNT_NUMBER = process.env.EVM_MAINNET_ACCOUNT_NUMBER;

var config = {
  defaultNetwork: "hardhat",
  etherscan: {
    apiKey: ETHERSCAN_API_KEY,
  },

  sourcify: {
    enabled: true,
  },

  networks: {
    hardhat: {},

    development: {
      url: "http://127.0.0.1:8545",
      accounts: {
        mnemonic: DEV_MNEMONIC,
      },
      gas: 25e6, // Gas limit
      gasPrice: 20e9,
      deploymentConfig: {
        guardianIds: [
          "0x05501355922a6529670DB49158676D98D6c34245",
          "0x084321C892ebb289dA2131d18a39fdfC3CCC0D2C",
          "0xd7a898720ab24ae154d67f51F2F75341D2A3719f",
        ],
        threshold: 2,
        governanceIds: [
          "0x05501355922a6529670DB49158676D98D6c34245",
          "0x084321C892ebb289dA2131d18a39fdfC3CCC0D2C",
          "0xd7a898720ab24ae154d67f51F2F75341D2A3719f",
        ],
        governanceThreshold: 2,
        tokenConfigPath: "../cfg/tokens_dev.json",
      },
    },

    bridgenet: {
      url: "https://rpc-eth-bridgenet.dev.azero.dev",
      accounts: {
        mnemonic: DEV_MNEMONIC,
      },
      governanceThreshold: 2,
      chainId: 12_345,
      gas: 25e6, // Gas limit
      gasPrice: 20e9,
      deploymentConfig: {
        guardianIds: [
          "0x05501355922a6529670DB49158676D98D6c34245",
          "0x084321C892ebb289dA2131d18a39fdfC3CCC0D2C",
          "0xd7a898720ab24ae154d67f51F2F75341D2A3719f",
        ],
        threshold: 2,
        governanceIds: [
          "0x05501355922a6529670DB49158676D98D6c34245",
          "0x084321C892ebb289dA2131d18a39fdfC3CCC0D2C",
          "0xd7a898720ab24ae154d67f51F2F75341D2A3719f",
        ],
        governanceThreshold: 2,
        tokenConfigPath: "../cfg/tokens_dev.json",
      },
    },
  },
  solidity: {
    compilers: [
      {
        version: "0.8.20",
        settings: {
          optimizer: {
            enabled: true,
            runs: 200,
          },
        },
      },
      {
        version: "0.4.18",
        settings: {
          optimizer: {
            enabled: true,
            runs: 200,
          },
        },
      },
    ],
  },
  paths: {
    sources: "./contracts",
    tests: "./test",
    cache: "./cache",
    artifacts: "./artifacts",
  },
  mocha: {
    timeout: 40_000,
  },
};

var SEPOLIA_ACCOUNT;

if (SEPOLIA_MNEMONIC) {
  SEPOLIA_ACCOUNT = { mnemonic: SEPOLIA_MNEMONIC };
} else if (SEPOLIA_PRIVATE_KEY) {
  SEPOLIA_ACCOUNT = [SEPOLIA_PRIVATE_KEY];
}

if (SEPOLIA_ACCOUNT) {
  config.networks.sepolia = {
    url:
      typeof SEPOLIA_URL == "undefined" || SEPOLIA_URL == ""
        ? "https://ethereum-sepolia-rpc.publicnode.com"
        : SEPOLIA_URL,
    accounts: SEPOLIA_ACCOUNT,
    deploymentConfig: {
      guardianIds: [
        typeof SEPOLIA_ACCOUNT_NUMBER == "undefined" ||
        SEPOLIA_ACCOUNT_NUMBER == ""
          ? "0xc4E0B92Df2DE77C077D060e49ec63DC196980716"
          : SEPOLIA_ACCOUNT_NUMBER, // sepolia account address corresponding to SEPOLIA_KEY
      ],
      threshold:
        typeof SEPOLIA_THRESHOLD == "undefined" || SEPOLIA_THRESHOLD == ""
          ? 1
          : parseInt(SEPOLIA_THRESHOLD, 10),
      weth:
        typeof SEPOLIA_WETH == "undefined" || SEPOLIA_WETH == ""
          ? "0xd91aE8FD2Be53F74876a9cc4aFb416645A0c8420"
          : SEPOLIA_WETH,
      tokenConfigPath:
        typeof SEPOLIA_TOKEN_CONFIG_PATH == "undefined" ||
        SEPOLIA_TOKEN_CONFIG_PATH == ""
          ? "../cfg/tokens_testnet_example.json"
          : SEPOLIA_TOKEN_CONFIG_PATH,
    },
  };
}

if (EVM_TESTNET_KEY) {
  config.networks.evm_testnet = {
    url: "https://rpc.alephzero-testnet.gelato.digital",
    accounts: [EVM_TESTNET_KEY],
    deploymentConfig: {
      dev: false,
      guardianIds: [
        typeof EVM_TESTNET_ACCOUNT_NUMBER == "undefined" ||
        EVM_TESTNET_ACCOUNT_NUMBER == ""
          ? "0x5027E6E6548b2eb986D4CC440C2a0dBB05D88946"
          : EVM_TESTNET_ACCOUNT_NUMBER, // L2 (evm testnet) account address corresponding to EVM_TESTNET_KEY
      ],
      threshold: 1,
      bazero_decimals: 12,
      bazero_token_mint: 10_000,
      pool_config: {
        amplification_coefficient: 2000,
        admin_fee: 0,
        fee: 0,
        initial_token_supply: 10_000,
        initial_native_supply: 10_000,
      },
      wazero_l1_address: "5EFDb7mKbougLtr5dnwd5KDfZ3wK55JPGPLiryKq4uRMPR46",
    },
  };
}

if (EVM_MAINNET_KEY) {
  config.networks.evm_mainnet = {
    url: "https://rpc.alephzero.raas.gelato.cloud",
    accounts: [EVM_MAINNET_KEY],
    deploymentConfig: {
      dev: false,
      guardianIds: [
        typeof EVM_MAINNET_ACCOUNT_NUMBER == "undefined" ||
        EVM_MAINNET_ACCOUNT_NUMBER == ""
          ? "0x66325D6a4C234b3927f9a9CA7Be510469c81e135"
          : EVM_MAINNET_ACCOUNT_NUMBER, // EVM account address corresponding to EVM_MAINNET_KEY
      ],
      threshold: 1,
      bazero_decimals: 12,
      bazero_token_mint: 100_000,
      pool_config: {
        amplification_coefficient: 2000,
        admin_fee: 0,
        fee: 0,
        initial_token_supply: 100_000,
        initial_native_supply: 100_000,
      },
      wazero_l1_address: "5CtuFVgEUz13SFPVY6s2cZrnLDEkxQXc19aXrNARwEBeCXgg",
    },
  };
}

if (ETHEREUM_PRIVATE_KEY) {
  config.networks.mainnet = {
    url: "https://ethereum-rpc.publicnode.com",
    accounts: [ETHEREUM_PRIVATE_KEY],
    deploymentConfig: {
      guardianIds: [
        ETHEREUM_GUARDIAN_ADDRESS, // Mainnet account address corresponding to ETHEREUM_PRIVATE_KEY
      ],
      threshold: 1,
      weth: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2",
    },
  };
}

module.exports = config;
