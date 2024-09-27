// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractMost} from "./AbstractMost.sol";
import {ITransferLimit} from "./ITransferLimit.sol";

/// @title Most
/// @author Cardinal Cryptography
contract Most is AbstractMost {
    ITransferLimit public transferLimit;

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function setTransferLimitContract(
        ITransferLimit _transferLimit
    ) external onlyOwner {
        transferLimit = _transferLimit;
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

    function checkTransferAllowed(
        address token,
        uint256 amount
    ) internal view override {
        if (transferLimit != ITransferLimit(address(0))) {
            try transferLimit.isRequestAllowed(token, amount) returns (
                bool result
            ) {
                if (!result) {
                    revert LimitExceeded();
                }
            } catch {
                // Ignore - behave as if the transferLimit is not set if it doesn't work for any reason
            }
        }
    }
}
