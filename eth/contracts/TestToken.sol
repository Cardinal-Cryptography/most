// Basic ERC20 token

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract TestToken is ERC20 {
    constructor() ERC20("TestToken", "TST") {
        // mint 10000 tokens to deployer
        _mint(msg.sender, 10000);
    }
}
