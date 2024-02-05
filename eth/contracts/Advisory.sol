// Most Advisory
//
// Interface + contract implementation

// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";

interface IAdvisory {
  /**
   * @dev Flips the emergency status.
   */
  function flipEmergency() external;

  /**
   * @dev Returns the current emergency status.
   */
  function isEmergency() external view returns (bool);
}

contract Advisory is IAdvisory, Ownable {
  bool public emergency;

  event EmergencyChanged(bool emergency);
  
  constructor(address initialOwner) Ownable(initialOwner) {
    emergency = false;
  }

  function flipEmergency() external onlyOwner {
    emergency = !emergency;
    emit EmergencyChanged(emergency);
  }

  function isEmergency() external view returns (bool) {
    return emergency;
  }

}
