// SPDX-License-Identifier: MIT

// Bridge entrypoint without ERC20 transfers

pragma solidity >=0.6.0;
pragma experimental ABIEncoderV2;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract BenchmarkSignatureAggregation {
    mapping(address => bool) private tokenWhitelist;
    mapping(address => bool) private guardians;
    mapping(bytes32 => bool) private signatures;
    mapping(bytes32 => uint) private signatureCount;
    uint private nonce;
    uint private external_nonce;
    uint private threshold;

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
    ) {
        require(_guardians.length > 0, "Must have at least one guardian");
        require(_threshold > 0 && _threshold <= _guardians.length, "Invalid threshold");

        for (uint i = 0; i < _tokenWhitelist.length; i++) {
            tokenWhitelist[_tokenWhitelist[i]] = true;
        }

        for (uint i = 0; i < _guardians.length; i++) {
            guardians[_guardians[i]] = true;
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
        IERC20(token).transferFrom(msg.sender, address(this), amount);

        emit BridgeTransferSuccess(token, to, amount, external_nonce++);
    }

    function bridgeReceive (
        address token,
        address beneficiary,
        uint amount,
        uint _nonce
    ) public {
        require(tokenWhitelist[token], "Selected token cannot be bridged");
        require(guardians[msg.sender], "Invalid guardian");
        require(_nonce == nonce, "Invalid nonce");

        bytes32 dataHash = keccak256(abi.encodePacked(nonce, token, beneficiary, amount));
        bytes32 signatureId = keccak256(abi.encodePacked(dataHash, msg.sender));
        
        require(!signatures[signatureId], "This guardian has already signed");

        if (signatureCount[dataHash] != threshold - 1) {
            signatures[signatureId] = true;
            signatureCount[dataHash]++;
            return;
        }

        signatureCount[dataHash] = 0;

        // Success, we can now transfer the tokens
        emit BridgeReceiveSuccess(token, beneficiary, amount, nonce++);
        IERC20(token).transfer(beneficiary, amount);
    }
}