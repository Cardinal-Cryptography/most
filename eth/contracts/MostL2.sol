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

    event NativeTransferFailed(bytes32 requestHash);
    event NativeTransferSwap(bytes32 requestHash, uint256 amount_out);
    event SwapFailed(bytes32 requestHash, uint256 amount_in);

    error SwapError();

    /// @custom:oz-upgrades-unsafe-allow constructor
    constructor() {
        _disableInitializers();
    }

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner,
        address payable _stableSwapAddress,
        address _bAzeroAddress
    ) public initializer {
        stableSwapAddress = _stableSwapAddress;
        bAzeroAddress = _bAzeroAddress;

        // Set the weth address to zero address. We dont use this in L2 most
        __AbstractMost_init(
            _committee,
            _signatureThreshold,
            payable(address(0))
        );
        __Ownable_init(owner);
        __Pausable_init();

        _pause();
    }

    function swap_from_bazero(uint256 amount) internal returns (bool, uint256) {
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        StableSwapTwoPool stablePool = StableSwapTwoPool(stableSwapAddress);

        // Allow swap to spend that many tokens
        bazero.approve(address(stableSwapAddress), amount);
        // At least half of amount_in
        uint256 min_amount_out = amount / 2;

        (bool swapSuccess, bytes memory returndata) = address(stablePool).call(
            abi.encodeCall(
                stablePool.exchange_to_native,
                (amount, min_amount_out)
            )
        );
        if (swapSuccess) {
            uint256 amount_out = abi.decode(returndata, (uint256));
            return (swapSuccess, amount_out);
        }
        return (swapSuccess, 0);
    }

    function swap_for_bazero(uint256 amount) internal returns (uint256) {
        StableSwapTwoPool stablePool = StableSwapTwoPool(stableSwapAddress);
        uint256 min_amount_out = amount / 2;
        return stablePool.exchange_from_native{value: amount}(min_amount_out);
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
            // So what we do here is:
            // 1. Mint `amount` Bazero
            // 2. Allow spending that many Bazero for swap contract
            // 3. exchange bazero for native tokens, here the swap spends its allowance and sends native to this contract
            // 4. transfer exchanged native to the receiver.
            IWrappedToken bazero = IWrappedToken(_destTokenAddress);
            bazero.mint(address(this), amount);

            (bool swapSuccess, uint256 amount_out) = swap_from_bazero(amount);

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

    function burn_bazero(uint256 amount) internal {
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        bazero.burn(amount);
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to send native Azero which are swapped for bazero
    /// tokens.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequestNative(
        bytes32 destReceiverAddress
    ) external payable override whenNotPaused {
        uint256 amount = msg.value;
        require(amount != 0, "Zero amount");
        if (amount == 0) revert ZeroAmount();
        require(destReceiverAddress != bytes32(0), "revert ZeroAddress()");

        bytes32 destTokenAddress = supportedPairs[
            addressToBytes32(bAzeroAddress)
        ];

        require(destTokenAddress != 0x0, "revert UnsupportedPair()");

        uint256 amount_out = swap_for_bazero(amount);
        burn_bazero(amount_out);

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount_out,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    function setBridgedAzeroAddress(
        address _bAzeroAddress
    ) external onlyOwner whenPaused {
        bAzeroAddress = _bAzeroAddress;
    }

    /// @dev Accept ether only from pool contract or through payable methods
    receive() external payable override {
        require(msg.sender == stableSwapAddress);
    }
}
