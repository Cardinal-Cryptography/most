// Basic ERC20 token

// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract SimpleToken is ERC20 {
    // contract admin
    address public admin;

    constructor() ERC20("SimpleToken", "SIM") {
        admin = msg.sender;
        // mint 10000 tokens to admin
        _mint(admin, 10000);
    }
}