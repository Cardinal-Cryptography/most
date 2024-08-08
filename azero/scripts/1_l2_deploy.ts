import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { KeyringPair } from "@polkadot/keyring/types";
import MostL2Constructors from "../types/constructors/most_l2";
import TokenConstructors from "../types/constructors/token";
import OracleConstructors from "../types/constructors/oracle";
import AdvisoryConstructors from "../types/constructors/advisory";
import WrappedAzeroConstructors from "../types/constructors/wrapped_azero";
import {
    estimateContractInit,
    import_env,
    storeAddresses,
    Addresses,
    import_token_config,
    findTokenBySymbol,
} from "./utils";
import "dotenv/config";
import "@polkadot/api-augment";
import { AccountId } from "../types/types-arguments/token";
import type BN from "bn.js";
import fs from "fs";

const envFile = process.env.AZERO_ENV;

async function main(): Promise<void> {
    if (!envFile) {
        throw new Error("Please provide an env file");
    }

    const config = await import_env(envFile);
    const tokenConfigPath = config.token_config_path;
    const tokenConfig = await import_token_config(tokenConfigPath);

    const isWrappedAzeroDeployed = findTokenBySymbol(
        "wAZERO",
        tokenConfig.aleph,
    ).deployed;

    const {
        ws_node,
        relayers,
        deployer_seed,
        signature_threshold,
    } = config;

    const wsProvider = new WsProvider(ws_node);
    const keyring = new Keyring({ type: "sr25519" });

    const api = await ApiPromise.create({ provider: wsProvider });
    const deployer = keyring.addFromUri(deployer_seed);
    console.log("Using", deployer.address, "as the deployer");

    const wrappedAzeroConstructors = new WrappedAzeroConstructors(api, deployer);
    const mostL2Constructors = new MostL2Constructors(api, deployer);
    const advisoryConstructors = new AdvisoryConstructors(api, deployer);

    var wrappedAzeroAddress = "";
    if (!isWrappedAzeroDeployed) {
        let estimatedGasWrappedAzero = await estimateContractInit(
            api,
            deployer,
            "wrapped_azero.contract",
            [],
        );

        wrappedAzeroAddress = await wrappedAzeroConstructors
            .new({
                gasLimit: estimatedGasWrappedAzero,
            })
            .then((res) => res.address);
    } else {
        wrappedAzeroAddress = findTokenBySymbol("wAZERO", tokenConfig.aleph).address;
    }

    let estimatedGasAdvisory = await estimateContractInit(
        api,
        deployer,
        "advisory.contract",
        [deployer.address],
    );

    const { address: advisoryAddress } = await advisoryConstructors.new(
        deployer.address, // owner
        { gasLimit: estimatedGasAdvisory },
    );

    const estimatedGasMostL2 = await estimateContractInit(
        api,
        deployer,
        "most_l2.contract",
        [
            relayers,
            signature_threshold!,
            deployer.address,
        ],
    );

    const { address: mostL2Address } = await mostL2Constructors.new(
        relayers,
        signature_threshold!,
        deployer.address,
        { gasLimit: estimatedGasMostL2 },
    );

    console.log("most address:", mostL2Address);

    var alephTokenAddresses = [];

    if (!isWrappedAzeroDeployed) {
        alephTokenAddresses.push({
            symbol: "wAZERO",
            address: wrappedAzeroAddress,
        });
    }

    const addresses = {
        mostL2: mostL2Address,
        advisory: advisoryAddress,
        alephTokens: alephTokenAddresses,
    };
    console.log("addresses:", addresses);

    fs.writeFileSync(
        __dirname + "/../l2_addresses.json",
        JSON.stringify(addresses, null, 2),
    );

    await api.disconnect();
    console.log("Done");
}

main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
});
