// Basic ERC20 token

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Token is ERC20 {
    uint8 private _decimals_;

    constructor(
        uint256 _totalSupply,
        uint8 _decimals,
        string memory _name,
        string memory _symbol
    ) ERC20(_name, _symbol) {
        _decimals_ = _decimals;
        _mint(msg.sender, _totalSupply);
    }

    function decimals() public view virtual override returns (uint8) {
        return _decimals_;
    }
}
