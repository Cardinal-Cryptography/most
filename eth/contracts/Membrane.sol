// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/presets/ERC20PresetMinterPauser.sol";

contract Membrane {
    uint256 public requestNonce = 0;

    uint256 public signatureThreshold = 1;

    struct Request {
        bytes32 destTokenAddress;
        uint256 destTokenAmount;
        bytes32 destReceiverAddress;
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
        bytes32 indexed destChainId,
        bytes32 indexed destTokenAddress,
        uint256 destTokenAmount,
        bytes32 destReceiverAddress,
        uint256 requestNonce
    );

    event RequestProcessed(bytes32 requestHash);

    modifier onlyGuardian() {
        require(isGuardian(msg.sender), "Can only be called by a signer");
        _;
    }

    function isGuardian(address sender) public view returns (bool) {
        return guardians[sender];
    }

    function bytes32toAddress(bytes32 data) internal pure returns (address) {
        return address(uint160(uint256(data)));
    }

    // Users invoke this tx to transfer funds to the destination chain.
    // Account need to approve this contract to spend the srcTokenAmount
    // on their behalf before executing the tx.
    //
    // Tx emits a CrosschainTransferRequest event that the relayers listen to
    // & forward to the destination chain.
    function sendRequest(
        address srcTokenAddress,
        uint256 srcTokenAmount,
        bytes32 destChainId,
        bytes32 destTokenAddress,
        uint256 destTokenAmount,
        bytes32 destReceiverAddress
    ) external {
        address sender = msg.sender;

        IERC20 token = IERC20(srcTokenAddress);

        // message sender needs to give approval else this tx will revert
        token.transferFrom(sender, address(this), srcTokenAmount);

        emit CrosschainTransferRequest(
            sender,
            srcTokenAddress,
            srcTokenAmount,
            destChainId,
            destTokenAddress,
            destTokenAmount,
            destReceiverAddress,
            requestNonce
        );

        requestNonce++;
    }

    // aggregates relayer signatures and burns/mints the token
    function receiveRequest(
        bytes32 requestHash,
        bytes32 destTokenAddress,
        uint256 destTokenAmount,
        bytes32 destReceiverAddress
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

        if (request.signatureCount >= signatureThreshold) {
            processedRequests[requestHash] = true;
            ERC20PresetMinterPauser token = ERC20PresetMinterPauser(
                bytes32toAddress(destTokenAddress)
            );
            token.mint(bytes32toAddress(destReceiverAddress), destTokenAmount);
            emit RequestProcessed(requestHash);
        }
    }

    // TODO aggregates sigs
    // TODO handle successful or failed requests
    // TODO introduce failure codes
    // TODO return funds for failed requests
    // TODO purge processed requests
    function acknowledgeRequest() external onlyGuardian {
        // TODO
    }

    function addGuardian() external onlyGuardian {
        // TODO
    }

    function removeGuardian() external onlyGuardian {
        // TODO
    }
}
