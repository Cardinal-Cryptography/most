// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";

contract MembraneProxy {

  address public admin;
  address public proxy;

  constructor(address _implementation, address _admin) {
    admin = _admin;
    TransparentUpgradeableProxy proxyInstance = new TransparentUpgradeableProxy(_implementation, admin, "");
    proxy = address(proxyInstance);
  }
}
