// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

/// @title MockOracle
/// @author Cardinal Cryptography
/// @notice Mock Chainlink oracle for testing purposes
contract MockOracle {
    int public price;

    function setPrice(int _price) public {
        price = _price;
    }

    function latestRoundData()
        public
        view
        returns (uint80, int, uint, uint, uint80)
    {
        return (0, price, 0, 0, 0);
    }

    function decimals() public pure returns (uint8) {
        return 8;
    }
}
