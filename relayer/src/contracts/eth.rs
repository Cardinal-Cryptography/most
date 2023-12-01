use ethers::contract::abigen;

abigen!(
    Membrane,
    "../eth/artifacts/contracts/Membrane.sol/Membrane.json"
);
