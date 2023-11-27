// SPDX-License-Identifier: MIT

pragma solidity ^0.8.0;

contract Governance {

  address public owner;
  uint256 public quorum;
  uint256 public nextId;

  struct Proposal {
    address destination;
    bytes payload;
    uint256 signatureCount;
    mapping(address => bool) signatures;
  }

    mapping(uint256 => Proposal) public pendingProposals;

  mapping(address => bool) private members;

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

  constructor(address[] memory _members,
              uint256 _quorum) {
    require(_members.length >= _quorum, "Not enough members specified");

    owner = msg.sender;
    quorum = _quorum;

    for (uint256 i = 0; i < _members.length; i++) {
      members[_members[i]] = true;
    }
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

    require(proposalExists(id), "NonExistentProposal");

    Proposal storage proposal = pendingProposals[id];

    require(proposal.signatures[msg.sender] == false, "ProposalAlreadySigned");

    proposal.signatureCount += 1;
    proposal.signatures[msg.sender] = true;

    emit Vote(msg.sender, id);
  }

  function executeProposal(uint256 id) external {

    require(proposalExists(id), "NonExistentProposal");

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

  function proposalExists(uint256 id) internal view returns (bool) {
    return pendingProposals[id].signatureCount > 0;
  }

  function hasSignedProposal(address member, uint256 id) external view returns (bool) {
    return pendingProposals[id].signatures[member];
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
