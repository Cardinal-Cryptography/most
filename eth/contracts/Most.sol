// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Ownable2StepUpgradeable} from "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {IWETH9} from "./IWETH9.sol";

/// @title Most
/// @author Cardinal Cryptography
contract Most is
    Initializable,
    UUPSUpgradeable,
    Ownable2StepUpgradeable,
    PausableUpgradeable
{
    using SafeERC20 for IERC20;

    /// @dev This amount of gas should be sufficient for ether transfers
    /// and simple fallback function execution, yet still protecting against reentrancy attack.
    uint256 constant GAS_LIMIT = 3500;

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

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner,
        address payable _wethAddress
    ) public initializer {
        committeeId = 0;
        wethAddress = _wethAddress;

        _setCommittee(_committee, _signatureThreshold);

        __Ownable_init(owner);
        __Pausable_init();
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
        bytes32 _srcTokenAddress,
        uint256 _amount,
        bytes32 _destReceiverAddress
    ) external whenNotPaused {
        if (_amount == 0) revert ZeroAmount();

        IERC20 token = IERC20(bytes32ToAddress(_srcTokenAddress));

        // lock tokens in this contract
        // message sender needs to give approval else this tx will revert
        token.safeTransferFrom(msg.sender, address(this), _amount);

        _sendRequest(
            _getRemoteTokenAddress(_srcTokenAddress),
            _amount,
            _destReceiverAddress
        );
    }

    /// @notice Invoke this tx to transfer funds to the destination chain.
    /// Account needs to send native ETH which are wrapped to wETH
    /// tokens.
    ///
    /// @dev Tx emits a CrosschainTransferRequest event that the relayers listen to
    /// & forward to the destination chain.
    function sendRequestNative(
        bytes32 _destReceiverAddress
    ) external payable whenNotPaused {
        uint256 _amount = msg.value;
        _sendRequestNative(_amount, _destReceiverAddress);
    }

    /// @notice Aggregates relayer signatures and returns the locked tokens.
    /// @dev When the ether is being bridged and the receiver is a contract
    /// that does not accept ether or fallback function consumes more than `GAS_LIMIT` gas units,
    /// the request is processed without revert and the ether is locked
    /// in this contract. Governance action must be taken to retrieve the tokens.
    function receiveRequest(
        bytes32 _requestHash,
        uint256 _committeeId,
        bytes32 destTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress,
        bytes32 _senderAddress,
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
                _senderAddress,
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
            processedRequests[requestHash] = true;
            delete pendingRequests[requestHash];

            address _destTokenAddress = bytes32ToAddress(destTokenAddress);
            address _destReceiverAddress = bytes32ToAddress(
                destReceiverAddress
            );
            if (_destTokenAddress == wethAddress) {
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

                    // In case of a failure return the locked ETH to the sender
                    _sendRequestNative(amount, _senderAddress);
                }
            } else {
                IERC20 token = IERC20(_destTokenAddress);
                if (
                    !tokenTransferReturnSuccess(
                        token,
                        _destReceiverAddress,
                        amount
                    )
                ) {
                    emit TokenTransferFailed(requestHash);

                    // In case of a failure return the locked tokens to the sender
                    _sendRequest(
                        _getRemoteTokenAddress(destTokenAddress),
                        amount,
                        _senderAddress
                    );
                }
            }
            emit RequestProcessed(requestHash);
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
    ) external onlyOwner {
        ++committeeId;
        _setCommittee(_committee, _signatureThreshold);
        emit CommitteeUpdated(committeeId);
    }

    function addPair(bytes32 from, bytes32 to) external onlyOwner whenPaused {
        supportedPairs[from] = to;
    }

    function removePair(bytes32 from) external onlyOwner whenPaused {
        delete supportedPairs[from];
    }

    function hasSignedRequest(
        address guardian,
        bytes32 hash
    ) external view returns (bool) {
        return pendingRequests[hash].signatures[guardian];
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

    function _getRemoteTokenAddress(
        bytes32 _tokenAddress
    ) private view returns (bytes32) {
        bytes32 remoteTokenAddress = supportedPairs[_tokenAddress];
        if (remoteTokenAddress == 0x0) revert UnsupportedPair();
        return remoteTokenAddress;
    }

    function _sendRequest(
        bytes32 _destTokenAddress,
        uint256 _amount,
        bytes32 _destReceiverAddress
    ) private {
        emit CrosschainTransferRequest(
            committeeId,
            _destTokenAddress,
            _amount,
            _destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    function _sendRequestNative(
        uint256 _amount,
        bytes32 _destReceiverAddress
    ) private {
        if (_amount == 0) revert ZeroAmount();

        bytes32 _destTokenAddress = _getRemoteTokenAddress(
            addressToBytes32(wethAddress)
        );

        (bool _success, ) = wethAddress.call{value: _amount}(
            abi.encodeCall(IWETH9.deposit, ())
        );
        if (!_success) revert WrappingEth();

        _sendRequest(_destTokenAddress, _amount, _destReceiverAddress);
    }

    /// @dev Adapted from Openzeppelin SafeERC20 - check if the ERC20 token transfer succeeded
    function tokenTransferReturnSuccess(
        IERC20 token,
        address receiver,
        uint256 amount
    ) private returns (bool) {
        (bool success, bytes memory returndata) = address(token).call(
            abi.encodeCall(token.transfer, (receiver, amount))
        );
        return
            success &&
            (returndata.length == 0 || abi.decode(returndata, (bool))) &&
            address(token).code.length > 0;
    }

    function isContract(address _addr) internal view returns (bool) {
        return _addr.code.length != 0;
    }

    /// @dev Accept ether only from weth contract or through payable methods
    receive() external payable {
        require(msg.sender == wethAddress);
    }
}
