// Basic ERC20 token

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract Poison is ERC20 {
    uint8 private _decimals;

    constructor(
        uint256 _totalSupply,
        uint8 _decimals_,
        string memory _name,
        string memory _symbol
    ) ERC20(_name, _symbol) {
        _decimals = _decimals_;
        _mint(msg.sender, _totalSupply);
    }

    function decimals() public view virtual override returns (uint8) {
        return _decimals;
    }

    function transfer(address, uint256) public virtual override returns (bool) {
        require(0 == 1, "Poison");
        return false;
    }
}
