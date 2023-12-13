// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts-upgradeable/utils/ContextUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

contract EmergencyHaltable is Initializable, ContextUpgradeable, OwnableUpgradeable {
    address public emergencyHalter;
    uint256 public lastEmergencyHalt;
    uint256 public emergencyHaltDuration;
    uint256 public emergencyHaltFrequency;

    modifier onlyEmergencyHalter() {
        require(msg.sender == emergencyHalter, "NotEmergencyHalter");
        _;
    }

    modifier noEmergencyHalt() {
        require(
            lastEmergencyHalt + emergencyHaltDuration < block.timestamp,
            "EmergencyHalt"
        );
        _;
    }

    function __EmergencyHaltable_init(address initialOwner) internal onlyInitializing {
        __Ownable_init(initialOwner);
    }

    function emergencyHalt() external onlyEmergencyHalter {
        require(
            block.timestamp > lastEmergencyHalt + emergencyHaltFrequency,
            "EmergencyHaltFrequency"
        );
        lastEmergencyHalt = block.timestamp;
    }

    function setEmergencyHaltParams(
        address _emergencyHalter,
        uint256 _emergencyHaltDuration,
        uint256 _emergencyHaltFrequency
    ) external onlyOwner {
        emergencyHalter = _emergencyHalter;
        emergencyHaltDuration = _emergencyHaltDuration;
        emergencyHaltFrequency = _emergencyHaltFrequency;
    }

    function resetEmergencyHaltTimestamp() external onlyOwner {
        lastEmergencyHalt = 0;
    }
}
