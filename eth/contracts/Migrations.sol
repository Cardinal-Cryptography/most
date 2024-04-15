// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

contract Migrations {
    address public immutable owner;
    uint256 public last_completed_migration;

    error CallerNotOwner(address);

    modifier restricted() {
        if (msg.sender != owner) revert CallerNotOwner(msg.sender);
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    function setCompleted(uint256 completed) public restricted {
        last_completed_migration = completed;
    }
}
