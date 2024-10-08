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

    /// Ratio between bridged azero (12 decimals) and native token on L2 (18 decimals)
    uint256 public constant BAZERO_TO_NATIVE_RATIO = 1e6;

    address payable public stableSwapAddress;
    address public bAzeroAddress;

    /// Rate of swap in stable swap we are content with
    uint256 public constant MIN_SWAP_RATE = 99;

    event NativeTransferFailed(bytes32 requestHash);
    event NativeTransferSwap(bytes32 requestHash, uint256 amount_out);
    event SwapFailed(bytes32 requestHash, uint256 amount_in);

    error SwapError();

    bytes32 internal constant EMPTY_STORAGE = 0x0;
    bytes32 internal constant NATIVE_MARKER_BYTES = 0x0;
    address internal constant NATIVE_MARKER_ADDRESS = address(0);

    /// flat fee paid upon requesting transfer
    uint256 public flat_fee;

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
        // Initial fee set to 0.5 Azero
        flat_fee = 1e18 / 2;

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
    /// Takes into account the difference in number of decimals between bazero and native token.
    function calc_min_amount_out_swap(
        uint256 amount,
        bool to_bazero
    ) internal pure returns (uint256) {
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
        // What we do here is:
        // 1. Mint `amount` Bazero
        // 2. Allow spending that many Bazero for swap contract
        // 3. exchange bazero for native tokens, here the swap spends its allowance and sends native to this contract
        // 4. transfer exchanged native to the receiver.
        IWrappedToken bazero = IWrappedToken(bAzeroAddress);
        bazero.mint(address(this), amount);

        (bool swapSuccess, uint256 amount_out) = swap_from_bazero(amount);

        if (!swapSuccess) {
            IERC20(bAzeroAddress).safeTransfer(_destReceiverAddress, amount);
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
            !isLocalToken[_destTokenAddress],
            "We dont bridge non local token"
        );

        // transfer native
        if (_destTokenAddress == NATIVE_MARKER_ADDRESS) {
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

    /// This function, if it is possible, transfer flat fee to owner
    /// and returns the surplus to the caller.
    function handle_flat_fee(uint256 native_amount) internal {
        uint256 transferred = msg.value;

        require(
            transferred >= native_amount,
            "Not enough value send for transfer"
        );

        transferred -= native_amount;
        require(transferred >= flat_fee, "Not enough value send for fees");

        uint256 surplus = transferred - flat_fee;

        (bool sent, ) = owner().call{value: flat_fee}("");
        require(sent, "Failed to send fee to owner");

        if (surplus > 0) {
            (bool sent, ) = msg.sender.call{value: surplus}("");
            require(sent, "Failed to return surplus");
        }
    }

    function sendRequestNative(
        bytes32
    ) external payable override whenNotPaused {
        revert(
            "Not supported on L2 bridge, use `sendRequestNative` with additional arg"
        );
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to send native Azero which are swapped for bazero
    /// tokens. Since the Bazero have 12 decimals and Azero have 18,
    /// user need to send at leas 10e6 tokens with this call.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequestNative(
        bytes32 destReceiverAddress,
        uint256 amount_to_bridge
    ) external payable whenNotPaused {
        require(
            amount_to_bridge >= BAZERO_TO_NATIVE_RATIO,
            "Value must be at least 10e6"
        );
        if (destReceiverAddress == bytes32(0)) {
            revert ZeroAddress();
        }
        handle_flat_fee(amount_to_bridge);

        uint256 amount_out = swap_for_bazero(amount_to_bridge);
        burn_bazero(amount_out);

        emit CrosschainTransferRequest(
            committeeId,
            NATIVE_MARKER_BYTES,
            amount_out,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    function sendRequest(
        bytes32,
        uint256,
        bytes32
    ) external override whenNotPaused {
        revert("Not supported on L2 bridge, use `SendTokenRequest` instead");
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to approve the Most contract to spend the `srcTokenAmount`
    /// of `srcTokenAddress` tokens on their behalf before executing the tx.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendTokenRequest(
        bytes32 srcTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) external payable whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (destReceiverAddress == bytes32(0)) revert ZeroAddress();
        handle_flat_fee(0);

        address token = bytes32ToAddress(srcTokenAddress);

        bytes32 destTokenAddress = supportedPairs[srcTokenAddress];

        if (destTokenAddress == EMPTY_STORAGE) revert UnsupportedPair();
        // Should not happen, see `addPair` function where we allow only nonLocal tokens.
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

    function addPair(
        bytes32 from,
        bytes32 to,
        bool isLocal
    ) external override onlyOwner whenPaused {
        require(!isLocal, "L2 Most dont bridge local tokens");
        supportedPairs[from] = to;
        isLocalToken[bytes32ToAddress(from)] = false;
    }

    function setFlatFee(uint256 new_flat_fee) external onlyOwner {
        flat_fee = new_flat_fee;
    }

    /// @dev Accept ether only from pool contract or through payable methods
    receive() external payable override {
        require(msg.sender == stableSwapAddress);
    }
}
