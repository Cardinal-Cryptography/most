// SPDX-License-Identifier: MIT

// Bridge entrypoint without ERC20 transfers

pragma solidity >=0.6.0;
pragma experimental ABIEncoderV2;

import {ECDSA} from "openzeppelin-solidity/contracts/cryptography/ECDSA.sol";

contract BenchmarkSignatureVerification {
    mapping(address => bool) private tokenWhitelist;
    mapping(address => uint) private guardians;
    uint private nonce;
    uint private external_nonce;
    uint private threshold;
    uint public constant MAX_GUARDIAN_COUNT = 20;

    event BridgeTransferSuccess(
        address indexed token, 
        string to, 
        uint amount,
        uint indexed nonce
    );

    event BridgeReceiveSuccess(
        address indexed token, 
        address beneficiary, 
        uint amount,
        uint indexed nonce
    );

    constructor(
        address[] memory _tokenWhitelist, 
        address[] memory _guardians, 
        uint _nonce,
        uint _external_nonce, 
        uint _threshold
    ) public {
        require(_guardians.length > 0, "Must have at least one guardian");
        require(_threshold > 0 && _threshold <= _guardians.length, "Invalid threshold");
        require(_guardians.length <= MAX_GUARDIAN_COUNT, "Too many guardians");

        for (uint i = 0; i < _tokenWhitelist.length; i++) {
            tokenWhitelist[_tokenWhitelist[i]] = true;
        }

        for (uint i = 0; i < _guardians.length; i++) {
            guardians[_guardians[i]] = i + 1;
        }

        nonce = _nonce;
        external_nonce = _external_nonce;
        threshold = _threshold;
    }

    function bridgeTransfer(
        address token,
        string calldata to,
        uint amount
    ) external {
        require(tokenWhitelist[token], "Selected token cannot be bridged");
        
        // transferFrom here

        emit BridgeTransferSuccess(token, to, amount, external_nonce++);
    }

    function bridgeReceive (
        address token,
        address beneficiary,
        uint amount,
        bytes[] memory signatures
    ) public {
        require(tokenWhitelist[token], "Selected token cannot be bridged");
        
        bytes32 dataHash = keccak256(abi.encodePacked(nonce, token, beneficiary, amount));
        verifyMultisig(dataHash, signatures);

        // Success, we can now transfer the tokens

        emit BridgeReceiveSuccess(token, beneficiary, amount, nonce++);
    }

    function verifyMultisig(bytes32 dataHash, bytes[] memory signatures) private view {
        require(signatures.length == threshold, "Incorrect number of signatures");
        
        bytes32 hashToSign = ECDSA.toEthSignedMessageHash(dataHash);

        bool[MAX_GUARDIAN_COUNT] memory signed;
        address signer;

        for (uint i = 0; i < signatures.length; i++) {
            signer = ECDSA.recover(hashToSign, signatures[i]);
            uint signerIndex = guardians[signer];
            require(signerIndex != 0 && !signed[signerIndex - 1], "Invalid signature");
            signed[signerIndex - 1] = true;
        }
    }
}