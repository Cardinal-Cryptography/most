// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractMost} from "./AbstractMost.sol";

/// @title Most
/// @author Cardinal Cryptography
contract L2Most is AbstractMost {
    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner,
        address payable _wethAddress
    ) public initializer {
        requestNonce = 0;
        committeeId = 0;
        wethAddress = _wethAddress;

        _setCommittee(_committee, _signatureThreshold);

        __Ownable_init(owner);
        __Pausable_init();
        _pause();
    }
}
