// SPDX-License-Identifier: MIT

pragma solidity ^0.8;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

contract Membrane {
  uint256 constant DIX_MILLE = 10000;

  address public owner;
  uint256 public requestNonce;
  uint256 public signatureThreshold;
  uint256 public commissionPerDixMille;
  uint256 public minimumTransferAmountUsd;
  uint256 public committeeId;
  bytes32 public USDT;
  bytes32 public WETH;

  struct Request {
    uint256 signatureCount;
    mapping(address => bool) signatures;
  }

  // from -> to mapping
  mapping(bytes32 => bytes32) public supportedPairs;
  mapping(bytes32 => Request) public pendingRequests;
  mapping(bytes32 => bool) public processedRequests;
  mapping(bytes32 => bool) private committee;
  mapping (uint256 => uint256) public committeeSize;
  mapping(bytes32 => uint256) private collectedCommitteeRewards;
  mapping(bytes32 => uint256) private paidOutMemberRewards;

  event CrosschainTransferRequest(
                                  bytes32 indexed destTokenAddress,
                                  uint256 amount,
                                  bytes32 destReceiverAddress,
                                  uint256 requestNonce
                                  );

  event RequestSigned(
                      bytes32 requestHash,
                      address signer
                      );

  event RequestProcessed(bytes32 requestHash);

  modifier _onlyOwner() {
    require(msg.sender == owner);
    _;
  }

  modifier _onlyCurrentCommitteeMember() {
    require(isInCommittee(committeeId, msg.sender), "Can only be called by a current committeMember");
    _;
  }

  constructor(
              address[] memory _committee,
              uint256 _signatureThreshold,
              uint256 _commissionPerDixMille,
              uint256 _minimumTransferAmountUsd
              ) {
    require(_signatureThreshold > 0, "Signature threshold must be greater than 0");
    require(_committee.length >= _signatureThreshold, "Not enough guardians specified");

    owner = msg.sender;
    signatureThreshold = _signatureThreshold;
    commissionPerDixMille = _commissionPerDixMille;
    minimumTransferAmountUsd = _minimumTransferAmountUsd;
    committeeId = 1;

    for (uint256 i = 0; i < _committee.length; i++) {
      committee[keccak256(abi.encodePacked(committeeId, _committee[i]))] = true;
    }

    committeeSize [committeeId] = _committee.length;
  }

  // Invoke this tx to transfer funds to the destination chain.
  // Account needs to approve the Membrane contract to spend the `srcTokenAmount`
  // of `srcTokenAddress` tokens on their behalf before executing the tx.
  //
  // Tx emits a CrosschainTransferRequest event that the relayers listen to
  // & forward to the destination chain.
  function sendRequest(
                       bytes32 srcTokenAddress,
                       uint256 amount,
                       bytes32 destReceiverAddress
                       ) external {

    require(queryPrice(amount, srcTokenAddress, USDT) > minimumTransferAmountUsd, "AmountBelowMinimum");

    address sender = msg.sender;

    IERC20 token = IERC20(bytes32ToAddress(srcTokenAddress));

    // check if the token is supported
    bytes32 destTokenAddress = supportedPairs[srcTokenAddress];
    require(destTokenAddress != 0x0, "Unsupported pair");

    // lock tokens in this contract
    // message sender needs to give approval else this tx will revert
    token.transferFrom(sender, address(this), amount);

    emit CrosschainTransferRequest(destTokenAddress,
                                   amount,
                                   destReceiverAddress,
                                   requestNonce);

    requestNonce++;
  }

  // aggregates relayer signatures and returns the locked tokens
  function receiveRequest(
                          bytes32 _requestHash,
                          bytes32 destTokenAddress,
                          uint256 amount,
                          bytes32 destReceiverAddress,
                          uint256 _requestNonce
                          ) external _onlyCurrentCommitteeMember {
    require(!processedRequests[_requestHash], "This request has already been processed");

    bytes32 requestHash = keccak256(abi.encodePacked(destTokenAddress,
                                                     amount,
                                                     destReceiverAddress,
                                                     _requestNonce));

    require(_requestHash == requestHash,
            "Hash does not match the data");

    Request storage request = pendingRequests[requestHash];
    require(!request.signatures[msg.sender], "This guardian has already signed this request");

    request.signatures[msg.sender] = true;
    request.signatureCount++;

    emit RequestSigned(requestHash, msg.sender);

    if (request.signatureCount >= signatureThreshold) {
      uint256 commission = (amount * commissionPerDixMille) / DIX_MILLE;

      collectedCommitteeRewards [keccak256(abi.encodePacked(committeeId, destTokenAddress))] = getCollectedCommitteeRewards (committeeId, destTokenAddress) + commission;

      processedRequests[requestHash] = true;
      delete pendingRequests[requestHash];

      // return the locked tokens
      IERC20 token = IERC20(bytes32ToAddress(destTokenAddress));

      token.transfer(bytes32ToAddress(destReceiverAddress), amount - commission);
      emit RequestProcessed(requestHash);

    }
  }

  // TODO
  // Request payout of rewards for signing & relaying cross-chain transfers
  //
  // Can be called by anyone on behalf of the committee member,
  // past or present
  function payoutRewards (
                          uint256 _committeeId,
                          address member,
                          bytes32 token
                          ) external {

    uint256 paidOutRewards = getPaidOutMemberRewards (_committeeId, member, token);
    uint256 outStandingRewards = getOutstandingMemberRewards (_committeeId, member, token);    

    if (outStandingRewards > 0) {


            /* token.transfer(bytes32ToAddress(destReceiverAddress), ); */

      paidOutMemberRewards [keccak256(abi.encodePacked(_committeeId, member, token))] += outStandingRewards;
            
                                  }

  }
  
  function getCollectedCommitteeRewards (
                                         uint256 _committeeId,
                                         bytes32 token
                                         ) public view returns (uint256) {
    return collectedCommitteeRewards [keccak256(abi.encodePacked(_committeeId, token))];
  }

  function getPaidOutMemberRewards (
                                    uint256 _committeeId,
                                    address member,
                                    bytes32 token
                                    ) public view returns (uint256) {
    return paidOutMemberRewards [keccak256(abi.encodePacked(_committeeId, member, token))];
  }

  function getOutstandingMemberRewards (
                                        uint256 _committeeId,
                                        address member,
                                        bytes32 token
                                        ) public view returns (uint256) {
    return (getCollectedCommitteeRewards (_committeeId, token) / committeeSize [_committeeId]) - getPaidOutMemberRewards (_committeeId, member, token);
  }

  // Queries a price oracle and returns the price of an `amount` number of the `ofToken` tokens.
  // TODO: this is a mocked method pending implementation
  function queryPrice(
                      uint256 amountOf,
                      bytes32 ofToken,
                      bytes32 inToken
                      ) private view returns(uint256) {

    if ((ofToken == WETH) && (inToken == USDT)) {
        return amountOf * 2;
    }

    if ((ofToken == USDT) && (inToken == WETH)) {
        return amountOf / 2;
    }

    return amountOf;
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
    return committee[keccak256(abi.encodePacked(_committeeId, account))] == true;
  }

  function bytes32ToAddress(bytes32 data) internal pure returns (address) {
    return address(uint160(uint256(data)));
  }

  function addressToBytes32(address addr) internal pure returns (bytes32) {
    return bytes32(uint256(uint160(addr)));
  }

  /* function addGuardian(address guardian) external _onlyOwner { */
  /*   guardians[guardian] = true; */
  /* } */

  /* function removeGuardian(address guardian) external _onlyOwner { */
  /*   guardians[guardian] = false; */
  /* } */

  function setOwner(address _owner) external _onlyOwner {
    owner = _owner;
  }

  function addPair(bytes32 from, bytes32 to) external _onlyOwner {
    supportedPairs[from] = to;
  }

  function removePair(bytes32 from) external _onlyOwner {
    delete supportedPairs[from];
  }

  function setUSDT(bytes32 _USDT) external _onlyOwner {
    USDT = _USDT;
  }

  function setWETH(bytes32 _WETH) external _onlyOwner {
    WETH = _WETH;
  }

}
