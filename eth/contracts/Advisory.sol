// Most Advisory
//
// Interface + contract implementation

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

interface IsAdvisory {
    function setEmergency() external;

    function isEmergency() external view returns (bool);

    function setOwner() external;

    function owner() external view returns address;;        
}
