// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractMost} from "./AbstractMost.sol";

/// @title MostL2.sol
/// @author Cardinal Cryptography
contract MostL2 is AbstractMost {
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
        __AbstractMost_init(_committee, _signatureThreshold, _wethAddress);
        __Ownable_init(owner);
        __Pausable_init();

        _pause();
    }
}
