// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

contract WrappedEther is ERC20 {
    constructor() ERC20("WrappedEther", "WETH") {}

    event Mint(address indexed _from, uint _amount);
    event Burn(address indexed _to, uint _amount);

    fallback() external payable {
        mint();
    }

    receive() external payable {
        mint();
    }

    function mint() public payable {
        _mint(msg.sender, msg.value);
        emit Mint(msg.sender, msg.value);
    }

    function burn(uint _amount) public {
        _burn(msg.sender, _amount);
        payable(msg.sender).transfer(_amount);
        emit Burn(msg.sender, _amount);
    }
}
