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

  event ProposalExecuted(address by, uint256 id, bytes data);

  modifier _onlyOwner() {
    require(msg.sender == owner);
    _;
  }

  modifier _onlyMember() {
    require(isMember(msg.sender), "NotMember");
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

    Proposal storage proposal = pendingProposals[id];

    require(proposal.signatures[msg.sender] == false, "ProposalAlreadySigned");

    proposal.signatureCount += 1;
    proposal.signatures[msg.sender] = true;

    emit Vote(msg.sender, id);
  }

  function executeProposal(uint256 id) external {

    Proposal storage proposal = pendingProposals[id];

    require(hasQuorum(id), "NoQuorum");

    (bool success, bytes memory result) = proposal.destination.call(proposal.payload);

    require(success, "ExecuteProposalFailed");

    delete pendingProposals[id];

    emit ProposalExecuted(msg.sender, id, result);
  }

  function hasQuorum(uint256 id) public view returns (bool) {
    return pendingProposals[id].signatureCount >= quorum;
  }

  function isMember(address _address) public view returns (bool) {
    return members[_address];
  }

  function getSignatureCount(uint256 id) external view returns (uint256) {
    return pendingProposals[id].signatureCount;
  }

  function setQuorum(uint256 _quorum) external _onlyOwner {
    quorum = _quorum;
  }

  function setOwner(address _owner) external _onlyOwner {
    owner = _owner;
  }

  function addMember(address member) external _onlyOwner {
    members[member] = true;
  }

  function removeMember(address member) external _onlyOwner {
    members[member] = false;
  }

}
