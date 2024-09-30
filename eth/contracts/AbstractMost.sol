// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Ownable2StepUpgradeable} from "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {IWETH9} from "./IWETH9.sol";
import {IWrappedToken} from "./IWrappedToken.sol";

/// @title Most
/// @author Cardinal Cryptography
abstract contract AbstractMost is
    Initializable,
    UUPSUpgradeable,
    Ownable2StepUpgradeable,
    PausableUpgradeable
{
    using SafeERC20 for IERC20;

    /// @dev This amount of gas should be sufficient for ether transfers
    /// and simple fallback function execution, yet still protecting against reentrancy attack.
    uint256 public constant GAS_LIMIT = 3500;

    uint256 public requestNonce;
    uint256 public committeeId;
    address payable public wethAddress;

    mapping(bytes32 from => bytes32 to) public supportedPairs;
    mapping(bytes32 requestHash => Request) public pendingRequests;
    mapping(bytes32 requestHash => bool) public processedRequests;
    /// @dev committeeMemberId = keccak256(abi.encodePacked(committeeId, comitteeMemberAddress))
    mapping(bytes32 committeeMemberId => bool) private committee;
    mapping(uint256 committeeId => uint256) public committeeSize;
    mapping(uint256 committeeId => uint256) public signatureThreshold;
    mapping(address => bool) public isLocalToken;

    address public wrappedAzeroAddress;

    struct Request {
        uint256 signatureCount;
        mapping(address => bool) signatures;
    }

    event CrosschainTransferRequest(
        uint256 indexed committeeId,
        bytes32 indexed destTokenAddress,
        uint256 amount,
        bytes32 indexed destReceiverAddress,
        uint256 requestNonce
    );

    event RequestSigned(bytes32 requestHash, address signer);

    event RequestProcessed(bytes32 requestHash);

    /// @notice Emitted when guardian signs a request that has already been processed
    event ProcessedRequestSigned(bytes32 requestHash, address signer);

    event RequestAlreadySigned(bytes32 requestHash, address signer);

    event EthTransferFailed(bytes32 requestHash);

    event TokenTransferFailed(bytes32 requestHash);

    event CommitteeUpdated(uint256 newCommitteeId);

    modifier _onlyCommitteeMember(uint256 _committeeId) {
        if (!isInCommittee(_committeeId, msg.sender)) revert NotInCommittee();
        _;
    }

    error NotInCommittee();
    error ZeroSignatureTreshold();
    error DuplicateCommitteeMember();
    error NotEnoughGuardians();
    error UnsupportedPair();
    error DataHashMismatch();
    error ZeroAmount();
    error WrappingEth();
    error UnwrappingEth();
    error EthTransfer();
    error ZeroAddress();
    error AzeroAddressNotSet();
    error LimitExceeded();

    function __AbstractMost_init(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address payable _wethAddress
    ) internal onlyInitializing {
        __AbstractMost_init_unchained(
            _committee,
            _signatureThreshold,
            _wethAddress
        );
    }

    function __AbstractMost_init_unchained(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address payable _wethAddress
    ) internal onlyInitializing {
        requestNonce = 0;
        committeeId = 0;
        wethAddress = _wethAddress;

        _setCommittee(_committee, _signatureThreshold);
    }

    /// @dev required by the OZ UUPS module
    function _authorizeUpgrade(address) internal override onlyOwner {}

    /// @dev disable possibility to renounce ownership
    function renounceOwnership() public virtual override onlyOwner {}

    function _setCommittee(
        address[] calldata _committee,
        uint256 _signatureThreshold
    ) internal {
        if (_signatureThreshold == 0) revert ZeroSignatureTreshold();
        if (_committee.length < _signatureThreshold)
            revert NotEnoughGuardians();

        for (uint256 i; i < _committee.length; ++i) {
            if (_committee[i] == address(0)) revert ZeroAddress();
            bytes32 committeeMemberId = keccak256(
                abi.encodePacked(committeeId, _committee[i])
            );
            // avoid duplicates
            if (committee[committeeMemberId]) {
                revert DuplicateCommitteeMember();
            }
            committee[committeeMemberId] = true;
        }

        committeeSize[committeeId] = _committee.length;
        signatureThreshold[committeeId] = _signatureThreshold;
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
    ) external virtual whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (destReceiverAddress == bytes32(0)) revert ZeroAddress();

        bytes32 destTokenAddress = supportedPairs[srcTokenAddress];
        if (destTokenAddress == 0x0) revert UnsupportedPair();

        address token = bytes32ToAddress(srcTokenAddress);
        checkTransferAllowed(token, amount);

        // burn or lock tokens in this contract
        // message sender needs to give approval else this tx will revert
        IERC20 tokenERC20 = IERC20(token);
        tokenERC20.safeTransferFrom(msg.sender, address(this), amount);

        if (!isLocalToken[token]) {
            IWrappedToken burnableToken = IWrappedToken(token);
            burnableToken.burn(amount);
        }

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to send native ETH which are wrapped to wETH
    /// tokens.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequestNative(
        bytes32 destReceiverAddress
    ) external payable virtual whenNotPaused {
        uint256 amount = msg.value;
        if (amount == 0) revert ZeroAmount();
        if (destReceiverAddress == bytes32(0)) revert ZeroAddress();

        bytes32 destTokenAddress = supportedPairs[
            addressToBytes32(wethAddress)
        ];

        if (destTokenAddress == 0x0) revert UnsupportedPair();
        checkTransferAllowed(wethAddress, amount);

        (bool success, ) = wethAddress.call{value: amount}(
            abi.encodeCall(IWETH9.deposit, ())
        );

        if (!success) revert WrappingEth();

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    function sendRequestAzeroToNative(
        uint256 amount,
        bytes32 destReceiverAddress
    ) external virtual whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        if (destReceiverAddress == bytes32(0)) revert ZeroAddress();
        if (wrappedAzeroAddress == address(0)) revert AzeroAddressNotSet();

        bytes32 wrappedAzeroAddressBytes32 = addressToBytes32(
            wrappedAzeroAddress
        );
        bytes32 destTokenAddress = supportedPairs[wrappedAzeroAddressBytes32];

        if (destTokenAddress == 0x0) revert UnsupportedPair();
        checkTransferAllowed(wrappedAzeroAddress, amount);

        IERC20 azeroToken = IERC20(wrappedAzeroAddress);
        azeroToken.safeTransferFrom(msg.sender, address(this), amount);
        IWrappedToken burnableToken = IWrappedToken(wrappedAzeroAddress);
        burnableToken.burn(amount);

        emit CrosschainTransferRequest(
            committeeId,
            0x0,
            amount,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    function onReceiveRequestThresholdMet(
        bytes32 requestHash,
        bytes32 destTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) internal virtual {
        processedRequests[requestHash] = true;
        delete pendingRequests[requestHash];

        address _destTokenAddress = bytes32ToAddress(destTokenAddress);
        address _destReceiverAddress = bytes32ToAddress(destReceiverAddress);

        // return the locked tokens
        // address(0) indicates bridging native ether
        if (_destTokenAddress == address(0)) {
            (bool unwrapSuccess, ) = wethAddress.call(
                abi.encodeCall(IWETH9.withdraw, (amount))
            );
            if (!unwrapSuccess) revert UnwrappingEth();
            (bool sendNativeEthSuccess, ) = _destReceiverAddress.call{
                value: amount,
                gas: GAS_LIMIT
            }("");
            if (!sendNativeEthSuccess) {
                emit EthTransferFailed(requestHash);
            }
        } else if (!isLocalToken[_destTokenAddress]) {
            // Mint representation of the remote token
            IWrappedToken mintableToken = IWrappedToken(_destTokenAddress);
            mintableToken.mint(_destReceiverAddress, amount);
        } else {
            IERC20 token = IERC20(_destTokenAddress);
            if (
                !tokenTransferReturnSuccess(token, _destReceiverAddress, amount)
            ) {
                emit TokenTransferFailed(requestHash);
            }
        }

        emit RequestProcessed(requestHash);
    }

    /// @notice Aggregates relayer signatures and returns the locked tokens.
    /// @dev When the ether is being bridged and the receiver is a contractRequestSigned
    /// that does not accept ether or fallback function consumes more than `GAS_LIMIT` gas units,
    /// the request is processed without revert and the ether is locked
    /// in this contract. Governance action must be taken to retrieve the tokens.
    function receiveRequest(
        bytes32 _requestHash,
        uint256 _committeeId,
        bytes32 destTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress,
        uint256 _requestNonce
    ) external whenNotPaused _onlyCommitteeMember(_committeeId) {
        // Don't revert if the request has already been processed as
        // such a call can be made during regular guardian operation.
        if (processedRequests[_requestHash]) {
            emit ProcessedRequestSigned(_requestHash, msg.sender);
            return;
        }

        bytes32 requestHash = keccak256(
            abi.encodePacked(
                _committeeId,
                destTokenAddress,
                amount,
                destReceiverAddress,
                _requestNonce
            )
        );

        Request storage request = pendingRequests[requestHash];
        if (request.signatures[msg.sender]) {
            emit RequestAlreadySigned(requestHash, msg.sender);
            return;
        }

        if (_requestHash != requestHash) revert DataHashMismatch();

        request.signatures[msg.sender] = true;
        ++request.signatureCount;

        emit RequestSigned(requestHash, msg.sender);

        if (request.signatureCount >= signatureThreshold[_committeeId]) {
            onReceiveRequestThresholdMet(
                requestHash,
                destTokenAddress,
                amount,
                destReceiverAddress
            );
        }
    }

    function pause() external onlyOwner {
        _pause();
    }

    function unpause() external onlyOwner {
        _unpause();
    }

    function recoverERC20(
        address token,
        address to,
        uint256 amount
    ) external onlyOwner {
        IERC20(token).safeTransfer(to, amount);
    }

    function recoverNative(
        address payable to,
        uint256 amount
    ) external onlyOwner {
        (bool success, ) = to.call{value: amount, gas: GAS_LIMIT}("");
        if (!success) revert EthTransfer();
    }

    function setCommittee(
        address[] calldata _committee,
        uint256 _signatureThreshold
    ) external onlyOwner whenPaused {
        ++committeeId;
        _setCommittee(_committee, _signatureThreshold);
        emit CommitteeUpdated(committeeId);
    }

    function setWrappedAzeroAddress(
        address _wrappedAzeroAddress
    ) external onlyOwner whenPaused {
        wrappedAzeroAddress = _wrappedAzeroAddress;
    }

    function addPair(
        bytes32 from,
        bytes32 to,
        bool isLocal
    ) external virtual onlyOwner whenPaused {
        supportedPairs[from] = to;
        isLocalToken[bytes32ToAddress(from)] = isLocal;
    }

    function setLocalToken(
        bytes32 token,
        bool isLocal
    ) external onlyOwner whenPaused {
        isLocalToken[bytes32ToAddress(token)] = isLocal;
    }

    function removePair(bytes32 from) external onlyOwner whenPaused {
        delete supportedPairs[from];
    }

    function hasSignedRequest(
        address guardian,
        bytes32 hash
    ) public view returns (bool) {
        return pendingRequests[hash].signatures[guardian];
    }

    function needsSignature(
        bytes32 requestHash,
        address account,
        uint256 _committeeId
    ) external view returns (bool) {
        if (!isInCommittee(_committeeId, account)) {
            return false;
        }
        if (processedRequests[requestHash]) {
            return false;
        }
        if (hasSignedRequest(account, requestHash)) {
            return false;
        }
        return true;
    }

    function isInCommittee(
        uint256 _committeeId,
        address account
    ) public view returns (bool) {
        return committee[keccak256(abi.encodePacked(_committeeId, account))];
    }

    function bytes32ToAddress(bytes32 data) internal pure returns (address) {
        return address(uint160(uint256(data)));
    }

    function addressToBytes32(address addr) internal pure returns (bytes32) {
        return bytes32(uint256(uint160(addr)));
    }

    /// @dev Adapted from Openzeppelin SafeERC20 - check if the ERC20 token transfer succeeded
    function tokenTransferReturnSuccess(
        IERC20 token,
        address receiver,
        uint256 amount
    ) internal returns (bool) {
        (bool success, bytes memory returndata) = address(token).call(
            abi.encodeCall(token.transfer, (receiver, amount))
        );
        return
            success &&
            (returndata.length == 0 || abi.decode(returndata, (bool))) &&
            address(token).code.length > 0;
    }

    /// @dev Accept ether only from weth contract or through payable methods
    receive() external payable virtual {
        require(msg.sender == wethAddress);
    }

    function checkTransferAllowed(
        address token,
        uint256 amount
    ) internal view virtual {}
}
