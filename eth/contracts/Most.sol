// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";

contract Most is Initializable, UUPSUpgradeable, Ownable2StepUpgradeable {
    uint256 public requestNonce;
    uint256 public committeeId;

    struct Request {
        uint256 signatureCount;
        mapping(address => bool) signatures;
    }

    // from -> to mapping
    mapping(bytes32 => bytes32) public supportedPairs;
    mapping(bytes32 => Request) public pendingRequests;
    mapping(bytes32 => bool) public processedRequests;
    mapping(bytes32 => bool) private committee;
    mapping(uint256 => uint256) public committeeSize;
    mapping(uint256 => uint256) public signatureThreshold;
    
    event CrosschainTransferRequest(
        uint256 indexed committeeId,
        bytes32 indexed destTokenAddress,
        uint256 amount,
        bytes32 indexed destReceiverAddress,
        uint256 requestNonce
    );

    event RequestSigned(bytes32 requestHash, address signer);

    event RequestProcessed(bytes32 requestHash);

    // Emitted when guardian signs a request that has already been processed
    event ProcessedRequestSigned(bytes32 requestHash, address signer);

    modifier _onlyCommitteeMember(uint256 _committeeId) {
        require(isInCommittee(_committeeId, msg.sender), "Not a member of the guardian committee");
        _;
    }

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner
    ) public initializer {
        require(
            _signatureThreshold > 0,
            "Signature threshold must be greater than 0"
        );
        require(
            _committee.length >= _signatureThreshold,
            "Not enough guardians specified"
        );

        committeeId = 0;

        for (uint256 i = 0; i < _committee.length; i++) {
            committee[
                keccak256(abi.encodePacked(committeeId, _committee[i]))
            ] = true;
        }

        committeeSize[committeeId] = _committee.length;
        signatureThreshold[committeeId] = _signatureThreshold;

        // inititialize the OwnableUpgradeable
        __Ownable_init(owner);
    }

    // required by the OZ UUPS module
    function _authorizeUpgrade(address) internal override onlyOwner {}

    // Disable possibility to renounce ownership
    function renounceOwnership() public virtual override onlyOwner {}

    // Invoke this tx to transfer funds to the destination chain.
    // Account needs to approve the Most contract to spend the `srcTokenAmount`
    // of `srcTokenAddress` tokens on their behalf before executing the tx.
    //
    // Tx emits a CrosschainTransferRequest event that the relayers listen to
    // & forward to the destination chain.
    function sendRequest(
        bytes32 srcTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) external {
        address sender = msg.sender;

        IERC20 token = IERC20(bytes32ToAddress(srcTokenAddress));

        // check if the token is supported
        bytes32 destTokenAddress = supportedPairs[srcTokenAddress];
        require(destTokenAddress != 0x0, "Unsupported pair");

        // lock tokens in this contract
        // message sender needs to give approval else this tx will revert
        token.transferFrom(sender, address(this), amount);

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount,
            destReceiverAddress,
            requestNonce
        );

        requestNonce++;
    }

    // aggregates relayer signatures and returns the locked tokens
    function receiveRequest(
        bytes32 _requestHash,
        uint256 _committeeId,
        bytes32 destTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress,
        uint256 _requestNonce
    ) external _onlyCommitteeMember(_committeeId) {
        // Don't revert if the request has already been processed as
        // such a call can be made during regular guardian operation.
        if (processedRequests[_requestHash]) {
            emit ProcessedRequestSigned(_requestHash, msg.sender);
            return;
        }

        bytes32 requestHash = keccak256(
            abi.encodePacked(
                _committeeId,
                destTokenAddress,
                amount,
                destReceiverAddress,
                _requestNonce
            )
        );

        require(_requestHash == requestHash, "Hash does not match the data");

        Request storage request = pendingRequests[requestHash];
        require(
            !request.signatures[msg.sender],
            "This guardian has already signed this request"
        );

        request.signatures[msg.sender] = true;
        request.signatureCount++;

        emit RequestSigned(requestHash, msg.sender);

        if (request.signatureCount >= signatureThreshold[committeeId]) {
            processedRequests[requestHash] = true;
            delete pendingRequests[requestHash];

            // return the locked tokens
            IERC20 token = IERC20(bytes32ToAddress(destTokenAddress));

            token.transfer(
                bytes32ToAddress(destReceiverAddress),
                amount
            );
            emit RequestProcessed(requestHash);
        }
    }

    function hasSignedRequest(
        address guardian,
        bytes32 hash
    ) external view returns (bool) {
        return pendingRequests[hash].signatures[guardian];
    }

    function isInCommittee(
        uint256 _committeeId,
        address account
    ) public view returns (bool) {
        return committee[keccak256(abi.encodePacked(_committeeId, account))];
    }

    function bytes32ToAddress(bytes32 data) internal pure returns (address) {
        return address(uint160(uint256(data)));
    }

    function addressToBytes32(address addr) internal pure returns (bytes32) {
        return bytes32(uint256(uint160(addr)));
    }

    function setCommittee(
        address[] memory _committee,
        uint256 _signatureThreshold
    ) external onlyOwner {
        require(
            _signatureThreshold > 0,
            "Signature threshold must be greater than 0"
        );
        require(
            _committee.length >= _signatureThreshold,
            "Not enough guardians specified"
        );

        committeeId += 1;

        for (uint256 i = 0; i < _committee.length; i++) {
            committee[
                keccak256(abi.encodePacked(committeeId, _committee[i]))
            ] = true;
        }

        committeeSize[committeeId] = _committee.length;
        signatureThreshold[committeeId] = _signatureThreshold;
    }

    function addPair(bytes32 from, bytes32 to) external onlyOwner {
        supportedPairs[from] = to;
    }

    function removePair(bytes32 from) external onlyOwner {
        delete supportedPairs[from];
    }
}
