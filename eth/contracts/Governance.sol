// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

contract Governance {

  address public owner;
  uint256 public quorum;
  uint256 public nextId;
  mapping(address => bool) members;
  mapping(uint256 => Proposal) pendingProposals;

  struct Proposal {
    address destination;
    bytes payload;
    uint256 signatureCount;
    mapping(address => bool) signatures;
  }

    event ProposalSubmitted(address by, uint256 id);

  event Vote(address by, uint256 id);

  event ProposalExecuted(address by, uint256 id);

  modifier _onlyOwner() {
    require(msg.sender == owner);
    _;
  }

  modifier _onlyMember() {
    require(isMember(msg.sender), "Can only be called by a comittee member");
    _;
  }

  constructor(uint256 _quorum) {
    owner = msg.sender;
    quorum = _quorum;
  }

  function submitProposal(address destination,
                          bytes calldata payload) external _onlyMember {

    Proposal storage proposal = pendingProposals[nextId];

    proposal.destination = destination;
    proposal.payload = payload;
    proposal.signatureCount = 1;
    proposal.signatures[msg.sender] = true;

    emit ProposalSubmitted(msg.sender, nextId);
    nextId += 1;
  }

  function vote(uint256 id) external _onlyMember {



  }

  function isMember(address _address) public view returns (bool) {
    return members[_address];
  }

  // TODO set owner

}
