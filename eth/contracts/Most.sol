// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {Initializable} from "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";
import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Ownable2StepUpgradeable} from "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";
import {PausableUpgradeable} from "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";
import {IWETH9} from "./IWETH9.sol";

contract Most is
    Initializable,
    UUPSUpgradeable,
    Ownable2StepUpgradeable,
    PausableUpgradeable
{
    using SafeERC20 for IERC20;

    uint256 public requestNonce;
    uint256 public committeeId;
    address payable public wethAddress;

    struct Request {
        uint256 signatureCount;
        mapping(address => bool) signatures;
    }

    // from -> to mapping
    mapping(bytes32 => bytes32) public supportedPairs;
    mapping(bytes32 => Request) public pendingRequests;
    mapping(bytes32 => bool) public processedRequests;
    mapping(bytes32 => bool) private committee;
    mapping(uint256 => uint256) public committeeSize;
    mapping(uint256 => uint256) public signatureThreshold;

    event CrosschainTransferRequest(
        uint256 indexed committeeId,
        bytes32 indexed destTokenAddress,
        uint256 amount,
        bytes32 indexed destReceiverAddress,
        uint256 requestNonce
    );

    event RequestSigned(bytes32 requestHash, address signer);

    event RequestProcessed(bytes32 requestHash);

    // Emitted when guardian signs a request that has already been processed
    event ProcessedRequestSigned(bytes32 requestHash, address signer);

    event RequestAlreadySigned(bytes32 requestHash, address signer);

    event EthTransferFailed(bytes32 requestHash);

    modifier _onlyCommitteeMember(uint256 _committeeId) {
        if (!isInCommittee(_committeeId, msg.sender)) revert NotInCommittee();
        _;
    }

    error NotInCommittee();
    error ZeroSignatureTreshold();
    error NotEnoughGuardians();
    error UnsupportedPair();
    error WrappingEth();
    error UnwrappingEth();
    error DataHashMismatch();
    error EthTransfer();
    error ZeroAmount();

    function initialize(
        address[] calldata _committee,
        uint256 _signatureThreshold,
        address owner,
        address payable _wethAddress
    ) public initializer {
        if (_signatureThreshold == 0) revert ZeroSignatureTreshold();
        if (_committee.length < _signatureThreshold)
            revert NotEnoughGuardians();

        committeeId = 0;

        for (uint256 i; i < _committee.length; ++i) {
            committee[
                keccak256(abi.encodePacked(committeeId, _committee[i]))
            ] = true;
        }

        committeeSize[committeeId] = _committee.length;
        signatureThreshold[committeeId] = _signatureThreshold;
        wethAddress = _wethAddress;
        __Ownable_init(owner);
        __Pausable_init();
    }

    function _authorizeUpgrade(address) internal override onlyOwner {
        // required by the OZ UUPS module
    }

    function renounceOwnership() public virtual override onlyOwner {
        // Disable possibility to renounce ownership
    }

    // Invoke this tx to transfer funds to the destination chain.
    // Account needs to approve the Most contract to spend the `srcTokenAmount`
    // of `srcTokenAddress` tokens on their behalf before executing the tx.
    //
    // Tx emits a CrosschainTransferRequest event that the relayers listen to
    // & forward to the destination chain.
    function sendRequest(
        bytes32 srcTokenAddress,
        uint256 amount,
        bytes32 destReceiverAddress
    ) external whenNotPaused {
        if (amount == 0) revert ZeroAmount();
        address sender = msg.sender;

        IERC20 token = IERC20(bytes32ToAddress(srcTokenAddress));

        // check if the token is supported
        bytes32 destTokenAddress = supportedPairs[srcTokenAddress];
        if (destTokenAddress == 0x0) revert UnsupportedPair();

        // lock tokens in this contract
        // message sender needs to give approval else this tx will revert
        token.safeTransferFrom(sender, address(this), amount);

        emit CrosschainTransferRequest(
            committeeId,
            destTokenAddress,
            amount,
            destReceiverAddress,
            requestNonce
        );

        ++requestNonce;
    }

    // Invoke this tx to transfer funds to the destination chain.
    // Account needs to send native ETH which are wrapped to wETH
    // tokens.
    //
    // Tx emits a CrosschainTransferRequest event that the relayers listen to
    // & forward to the destination chain.
    function sendRequestNative(bytes32 destReceiverAddress) external payable {
        uint256 amount = msg.value;
        if (amount == 0) revert ZeroAmount();

        // check if the token is supported
        bytes32 destTokenAddress = supportedPairs[
            addressToBytes32(wethAddress)
        ];

        if (destTokenAddress == 0x0) revert UnsupportedPair();

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

        requestNonce++;
    }

    // aggregates relayer signatures and returns the locked tokens
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
            processedRequests[requestHash] = true;
            delete pendingRequests[requestHash];

            // return the locked tokens
            address _destTokenAddress = bytes32ToAddress(destTokenAddress);
            if (_destTokenAddress == wethAddress) {
                (bool unwrapSuccess, ) = wethAddress.call(
                    abi.encodeCall(IWETH9.withdraw, (amount))
                );
                if (!unwrapSuccess) revert UnwrappingEth();
                address _destReceiverAddress = bytes32ToAddress(
                    destReceiverAddress
                );
                (bool sendNativeEthSuccess, ) = _destReceiverAddress.call{
                    value: amount,
                    gas: 3500
                }("");
                if (!sendNativeEthSuccess) {
                    if (isContract(_destReceiverAddress)) {
                        // fail without revert
                        emit EthTransferFailed(requestHash);
                    } else {
                        revert EthTransfer();
                    }
                }
            } else {
                IERC20 token = IERC20(_destTokenAddress);

                token.safeTransfer(bytes32ToAddress(destReceiverAddress), amount);
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

    function isContract(address _addr) internal view returns (bool) {
        return _addr.code.length != 0;
    }

    function setCommittee(
        address[] calldata _committee,
        uint256 _signatureThreshold
    ) external onlyOwner {
        if (_signatureThreshold == 0) revert ZeroSignatureTreshold();
        if (_committee.length < _signatureThreshold)
            revert NotEnoughGuardians();

        ++committeeId;

        for (uint256 i; i < _committee.length; ++i) {
            committee[
                keccak256(abi.encodePacked(committeeId, _committee[i]))
            ] = true;
        }

        committeeSize[committeeId] = _committee.length;
        signatureThreshold[committeeId] = _signatureThreshold;
    }

    function addPair(bytes32 from, bytes32 to) external onlyOwner {
        supportedPairs[from] = to;
    }

    function removePair(bytes32 from) external onlyOwner {
        delete supportedPairs[from];
    }

    // accept payments only from weth or through payable methods
    receive() external payable {
        require(msg.sender == wethAddress);
    }
}
