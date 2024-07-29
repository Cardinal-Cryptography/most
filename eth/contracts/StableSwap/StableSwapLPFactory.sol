// SPDX-License-Identifier: MIT
pragma solidity ^0.8.10;

import "@openzeppelin-4.5.0/contracts/access/Ownable.sol";
import "./StableSwapLP.sol";

contract StableSwapLPFactory is Ownable {
    event NewStableSwapLP(
        address indexed swapLPContract,
        address tokenA,
        address tokenB
    );

    constructor() {}

    /**
     * @notice createSwapLP
     * @param _tokenA: Addresses of ERC20 conracts .
     * @param _tokenB: Addresses of ERC20 conracts .
     * @param _minter: Minter address
     */
    function createSwapLP(
        address _tokenA,
        address _tokenB,
        address _minter
    ) external onlyOwner returns (address) {
        // create LP token
        bytes memory bytecode = type(StableSwapLP).creationCode;
        bytes32 salt = keccak256(
            abi.encodePacked(
                _tokenA,
                _tokenB,
                msg.sender,
                block.timestamp,
                block.chainid
            )
        );
        address lpToken;
        assembly {
            lpToken := create2(0, add(bytecode, 32), mload(bytecode), salt)
        }
        StableSwapLP(lpToken).setMinter(_minter);
        emit NewStableSwapLP(lpToken, _tokenA, _tokenB);
        return lpToken;
    }
}
