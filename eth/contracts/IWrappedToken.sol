// SPDX-License-Identifier: MIT
pragma solidity ^0.8.2;

/// @title Interface for WrappedToken
interface IWrappedToken {
    /// @notice Mint a given amount of remote PSP22 token representation to a given address
    function mint(address, uint256) external;

    /// @notice Burn a given amount of remote PSP22 token representation
    function burn(uint256) external;

    /// @notice Approve spedning a given amount of token to the spender
    function approve(address spender, uint256 value) external returns (bool);
}
