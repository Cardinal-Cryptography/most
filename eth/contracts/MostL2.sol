// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractMost} from "./AbstractMost.sol";
import {StableSwapTwoPool} from "./StableSwap/StableSwapTwoPool.sol";
import {IWrappedToken} from "./IWrappedToken.sol";

/// @title MostL2.sol
/// @author Cardinal Cryptography
contract MostL2 is AbstractMost {
    address payable public stableSwapAddress;
    address public bAzeroAddress;

    // todo: I have no idea...
    uint256 public constant SWAP_GAS_LIMIT = 3500;

    event NativeTransferFailed(bytes32 requestHash);
    event NativeTransferSwap(bytes32 requestHash, uint256 amount_out);
    event SwapFailed(
        bytes32 requestHash,
        uint256 amount_in
    );

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner,
        address payable _wethAddress,
        address payable _stableSwapAddress,
        address _bAzeroAddress
    ) public initializer {
        stableSwapAddress = _stableSwapAddress;
        bAzeroAddress = _bAzeroAddress;

        __AbstractMost_init(_committee, _signatureThreshold, _wethAddress);
        __Ownable_init(owner);
        __Pausable_init();

        _pause();
    }

    function swap_bazero(uint256 amount) internal returns(bool, uint256) {
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        // Allow swap to spend that many tokens
        bazero.approve(address(stableSwapAddress), amount);

        // At least half of amount_in
        uint256 min_amount_out = amount / 2;
        StableSwapTwoPool stablePool = StableSwapTwoPool(stableSwapAddress);
        (bool swapSuccess, bytes memory returndata) = address(stablePool)
            .call{gas: SWAP_GAS_LIMIT}(
            abi.encodeCall(
                stablePool.exchange_native,
                (amount, min_amount_out)
            )
        );
        uint256 amount_out = abi.decode(returndata, (uint256));

        return (swapSuccess, amount_out);
    }

    function onReceiveRequestThresholdMet(
        bytes32 requestHash,
        bytes32 destTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) internal override {
        processedRequests[requestHash] = true;
        delete pendingRequests[requestHash];

        address _destTokenAddress = bytes32ToAddress(destTokenAddress);
        address _destReceiverAddress = bytes32ToAddress(destReceiverAddress);

        require(
            _destTokenAddress != address(0),
            "We dont bridge native from L2"
        );
        require(
            !isLocalToken[_destTokenAddress],
            "We dont bridge non local token"
        );

        // transfer native
        if (_destTokenAddress == bAzeroAddress) {
            // Mint representation of the remote token to this contract
            IWrappedToken bazero = IWrappedToken(_destTokenAddress);
            bazero.mint(address(this), amount);

            // Allow swap to spend that many tokens
            bazero.approve(address(stableSwapAddress), amount);
            (bool swapSuccess, uint256 amount_out) = swap_bazero(amount);

            if (!swapSuccess) {
                emit SwapFailed(requestHash, amount);
                return;
            }

            // payout to receiver
            (bool sendNativeEthSuccess, ) = _destReceiverAddress.call{
                value: amount_out,
                gas: GAS_LIMIT
            }("");

            if (!sendNativeEthSuccess) {
                emit NativeTransferFailed(requestHash);
            } else {
                emit NativeTransferSwap(requestHash, amount_out);
            }
            emit RequestProcessed(requestHash);
        } else {
            // TODO non native transfer
        }
    }
}
