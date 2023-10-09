// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract Membrane {
    uint256 public requestNonce = 0;

    uint256 public signatureThreshold = 1;

    // TODO : getter @ongoing transfer requests

    struct Request {
        address destTokenAddress;
        uint256 destTokenAmount;
        address destReceiverAddress;
        uint256 signatureCount;
        mapping(address => bool) signatures;
    }

    mapping(bytes32 => Request) public pendingRequests;

    mapping(bytes32 => bool) public processedRequests;

    mapping(address => bool) private guardians;

    event CrosschainTransferRequest(
        address sender,
        address indexed srcTokenAddress,
        uint256 srcTokenAmount,
        string indexed destTokenAddress,
        string destReceiverAddress,
        uint256 requestNonce
    );

    event RequestProcessed(bytes32 requestHash, address signer);

    event RequestProcessed(bytes32 requestHash);

    modifier onlyGuardian() {
        require(isGuardian(msg.sender), "Can only be called by a signer");
        _;
    }

    constructor(
                address[] memory _guardians,
                uint256 _signatureThreshold
                ) {
        signatureThreshold = _signatureThreshold;
        for (uint256 i = 0; i < _guardians.length; i++) {
            guardians[_guardians[i]] = true;
        }
    }

    function isGuardian(address sender) public view returns (bool) {
        return guardians[sender];
    }

    // Invoke this tx to transfer funds to the destination chain.
    // Account needs to approve the Membrane contract to spend the srcTokenAmount
    // on their behalf before executing the tx.
    //
    // Tx emits a CrosschainTransferRequest event that the relayers listen to
    // & forward to the destination chain.
    function sendRequest(
        address srcTokenAddress,
        uint256 srcTokenAmount,
        string calldata destTokenAddress,
        string calldata destReceiverAddress
    ) external {
        address sender = msg.sender;

        IERC20 token = IERC20(srcTokenAddress);

        // message sender needs to give approval else this tx will revert
        token.transferFrom(sender, address(this), srcTokenAmount);

        emit CrosschainTransferRequest(
            sender,
            srcTokenAddress,
            srcTokenAmount,
            destTokenAddress,
            destReceiverAddress,
            requestNonce
        );

        requestNonce++;
    }

    // aggregates relayer signatures and burns/mints the token
    function receiveRequest(
        bytes32 requestHash,
        address destTokenAddress,
        uint256 destTokenAmount,
        address destReceiverAddress
    ) external onlyGuardian {
        require(
            !processedRequests[requestHash],
            "This request has already been processed"
        );

        Request storage request = pendingRequests[requestHash];
        if (request.signatureCount == 0) {
            request.destTokenAddress = destTokenAddress;
            request.destTokenAmount = destTokenAmount;
            request.destReceiverAddress = destReceiverAddress;
        }

        require(!request.signatures[msg.sender], "Already signed this request");

        request.signatures[msg.sender] = true;
        request.signatureCount++;

        emit RequestProcessed(requestHash, msg.sender);

        if (request.signatureCount >= signatureThreshold) {
            // We assume that nonce is hashed into the requestHash
            processedRequests[requestHash] = true;
            delete pendingRequests[requestHash];
            IERC20 token = IERC20(destTokenAddress);

            token.transfer(destReceiverAddress, destTokenAmount);
            
            emit RequestProcessed(requestHash);
        }
    }
}
