// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";

abstract contract EmergencyHaltable {
    function emergencyHalt() external virtual;
}
