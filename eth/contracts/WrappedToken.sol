// Wrapped representation of a remote PSP22 token that can be minted and burned

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {Ownable2Step} from "@openzeppelin/contracts/access/Ownable2Step.sol";
import {Ownable} from "@openzeppelin/contracts/access/Ownable.sol";

contract WrappedToken is ERC20, Ownable2Step {
    uint8 private _decimals;
    address public minterBurner;

    error NotMinter();
    error NotBurner();

    constructor(
        string memory _name,
        string memory _symbol,
        uint8 __decimals,
        address _minter_burner
    ) ERC20(_name, _symbol) Ownable(msg.sender) {
        minterBurner = _minter_burner;
        _decimals = __decimals;
    }

    modifier onlyMinter() {
        if (msg.sender != minterBurner) {
            revert NotMinter();
        }
        _;
    }

    modifier onlyBurner() {
        if (msg.sender != minterBurner) {
            revert NotBurner();
        }
        _;
    }

    function mint(address _to, uint256 _amount) external onlyMinter {
        _mint(_to, _amount);
    }

    function burn(uint256 _amount) external onlyBurner {
        _burn(msg.sender, _amount);
    }

    function setMinterBurner(address _minter_burner) external onlyOwner {
        minterBurner = _minter_burner;
    }

    function decimals() public view virtual override returns (uint8) {
        return _decimals;
    }
}
