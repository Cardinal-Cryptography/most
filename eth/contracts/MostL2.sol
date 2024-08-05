// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {AbstractMost} from "./AbstractMost.sol";
import {StableSwapTwoPool} from "./StableSwap/StableSwapTwoPool.sol";
import {IWrappedToken} from "./IWrappedToken.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";

/// @title MostL2.sol
/// @author Cardinal Cryptography
contract MostL2 is AbstractMost {
    using SafeERC20 for IERC20;

    /// Ration betweem bridged azero (12 decimals) and native token on L2 (18 decimals)
    uint256 public constant BAZERO_TO_NATIVE_RATIO = 10e6;

    address payable public stableSwapAddress;
    address public bAzeroAddress;

    /// Rate of swap in stable swap we are content with
    uint256 public constant MIN_SWAP_RATE = 99;

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

    /// Calculates min value of the swap we are happy with.
    /// Takes into the account decimals between bazero and native token.
    function calc_min_amount_out_swap(
        uint256 amount,
        bool to_bazero
    ) internal returns (uint256) {
        if (to_bazero) {
            return ((amount / 100) * MIN_SWAP_RATE) / BAZERO_TO_NATIVE_RATIO;
        } else {
            return (amount / 100) * MIN_SWAP_RATE * BAZERO_TO_NATIVE_RATIO;
        }
    }

    function swap_from_bazero(uint256 amount) internal returns (bool, uint256) {
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        StableSwapTwoPool stablePool = StableSwapTwoPool(stableSwapAddress);

        // Allow swap to spend that many tokens
        bazero.approve(address(stableSwapAddress), amount);
        // at least 99% of what we gave to the swap
        uint256 min_amount_out = calc_min_amount_out_swap(amount, false);

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
        return (false, 0);
    }

    function swap_for_bazero(uint256 amount) internal returns (uint256) {
        // at least 99% of what we gave to the swap
        uint256 min_amount_out = calc_min_amount_out_swap(amount, true);

        StableSwapTwoPool stablePool = StableSwapTwoPool(stableSwapAddress);
        return stablePool.exchange_from_native{value: amount}(min_amount_out);
    }

    function native_transfer(
        bytes32 requestHash,
        uint256 amount,
        address _destReceiverAddress
    ) internal {
        // So what we do here is:
        // 1. Mint `amount` Bazero
        // 2. Allow spending that many Bazero for swap contract
        // 3. exchange bazero for native tokens, here the swap spends its allowance and sends native to this contract
        // 4. transfer exchanged native to the receiver.
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
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
    }

    function remote_token_transfer(
        address _destTokenAddress,
        uint256 amount,
        address _destReceiverAddress
    ) internal {
        // Mint representation of the remote token
        IWrappedToken mintableToken = IWrappedToken(_destTokenAddress);
        mintableToken.mint(_destReceiverAddress, amount);
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
            native_transfer(requestHash, amount, _destReceiverAddress);
        } else {
            remote_token_transfer(
                _destTokenAddress,
                amount,
                _destReceiverAddress
            );
        }

        emit RequestProcessed(requestHash);
    }

    function burn_bazero(uint256 amount) internal {
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        bazero.burn(amount);
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to send native Azero which are swapped for bazero
    /// tokens. Since the Bazero have 12 decimals and Azero have 18,
    /// user need to send at leas 10e6 tokens with this call.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequestNative(
        bytes32 destReceiverAddress
    ) external payable override whenNotPaused {
        uint256 amount = msg.value;
        require(amount != 0, "Zero amount");
        if (amount == 0) revert ZeroAmount();
        require(
            amount >= BAZERO_TO_NATIVE_RATIO,
            "Value must be at least 10e6"
        );
        if (destReceiverAddress == bytes32(0)) {
            revert ZeroAddress();
        }

        bytes32 destTokenAddress = supportedPairs[
            addressToBytes32(bAzeroAddress)
        ];

        if (destTokenAddress == 0x0) {
            revert UnsupportedPair();
        }

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

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to approve the Most contract to spend the `srcTokenAmount`
    /// of `srcTokenAddress` tokens on their behalf before executing the tx.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequest(
        bytes32 srcTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) external override whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (destReceiverAddress == bytes32(0)) revert ZeroAddress();

        address token = bytes32ToAddress(srcTokenAddress);

        bytes32 destTokenAddress = supportedPairs[srcTokenAddress];
        if (destTokenAddress == 0x0) revert UnsupportedPair();
        require(
            !isLocalToken[token],
            "We dont bridge local tokens on L2 bridge"
        );

        // Burn tokens in this contract
        // message sender needs to give approval else this tx will revert
        IERC20 tokenERC20 = IERC20(token);
        tokenERC20.safeTransferFrom(msg.sender, address(this), amount);
        IWrappedToken burnableToken = IWrappedToken(token);
        burnableToken.burn(amount);

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount,
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

    function sendRequestAzeroToNative(uint256, bytes32) external pure override {
        revert("Not supported on L2 bridge");
    }

    /// @dev Accept ether only from pool contract or through payable methods
    receive() external payable override {
        require(msg.sender == stableSwapAddress);
    }
}
