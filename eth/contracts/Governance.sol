// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

contract Governance {

  address public owner;
  uint256 public quorum;
  mapping(address => bool) members;
  mapping(uint256 => Proposal) pendingProposals;

  struct Proposal {
    address destination;
    bytes payload;
    uint256 signatureCount;
    mapping(address => bool) signatures;
  }

  modifier _onlyOwner() {
    require(msg.sender == owner);
    _;
  }

  constructor(uint256 _quorum) {
    owner = msg.sender;
    quorum = _quorum;
  }

  // set owner

}
