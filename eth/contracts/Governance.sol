// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/OwnableUpgradeable.sol";

contract Governance is Initializable, UUPSUpgradeable, OwnableUpgradeable {
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

    modifier _onlyMember() {
        require(isMember(msg.sender), "NotMember");
        _;
    }

    function initialize(address[] calldata _members, uint256 _quorum) public initializer {
        require(_members.length >= _quorum, "Not enough members specified");

        quorum = _quorum;

        for (uint256 i = 0; i < _members.length; i++) {
            members[_members[i]] = true;
        }

        // inititialize the OwnableUpgradeable
        __Ownable_init(msg.sender);
    }

    // required by the OZ UUPS module
    function _authorizeUpgrade(address) internal override onlyOwner {}

    function submitProposal(
        address destination,
        bytes calldata payload
    ) external _onlyMember {
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

        require(
            proposal.signatures[msg.sender] == false,
            "ProposalAlreadySigned"
        );

        proposal.signatureCount += 1;
        proposal.signatures[msg.sender] = true;

        emit Vote(msg.sender, id);
    }

    function executeProposal(uint256 id) external {
        require(proposalExists(id), "NonExistentProposal");

        Proposal storage proposal = pendingProposals[id];

        require(hasQuorum(id), "NoQuorum");

        (bool success, bytes memory result) = proposal.destination.call(
            proposal.payload
        );

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

    function hasSignedProposal(
        address member,
        uint256 id
    ) external view returns (bool) {
        return pendingProposals[id].signatures[member];
    }

    function setQuorum(uint256 _quorum) external onlyOwner {
        quorum = _quorum;
    }

    function addMember(address member) external onlyOwner {
        members[member] = true;
    }

    function removeMember(address member) external onlyOwner {
        members[member] = false;
    }
}
