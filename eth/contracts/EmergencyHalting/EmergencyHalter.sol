// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "./EmergencyHaltable.sol";

contract EmergencyHalter is Ownable {
    mapping(address => bool) private halters;
    address public overseenContract;

    constructor(address[] memory _halters, address _overseenContract) Ownable(msg.sender) {
        for (uint256 i = 0; i < _halters.length; i++) {
            halters[_halters[i]] = true;
        }
        overseenContract = _overseenContract;
    }

    function halt() external {
        require(halters[msg.sender], "Not a halter");
        EmergencyHaltable(overseenContract).emergencyHalt();
    }

    function addHalter(address halter) external onlyOwner {
        halters[halter] = true;
    }

    function removeHalter(address halter) external onlyOwner {
        halters[halter] = false;
    }
}
