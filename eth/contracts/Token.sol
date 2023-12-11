// Basic ERC20 token

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Token is ERC20 {
    constructor(uint256 _totalSupply, string memory _name, string memory _symbol)
      ERC20(_name, _symbol) {
        _mint(msg.sender, _totalSupply);
    }
}
