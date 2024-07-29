// SPDX-License-Identifier: MIT
pragma solidity ^0.8.10;

import "@openzeppelin-4.5.0/contracts/access/Ownable.sol";
import "./interfaces/IStableSwapLP.sol";
import "./interfaces/IStableSwapDeployer.sol";
import "./interfaces/IStableSwapLPFactory.sol";

contract StableSwapFactory is Ownable {
    IStableSwapLPFactory public immutable LPFactory;
    IStableSwapDeployer public immutable SwapTwoPoolDeployer;

    address constant ZEROADDRESS = address(0);

    event NewStableSwapPair(
        address indexed swapContract,
        address tokenA,
        address tokenB,
        address LP
    );

    /**
     * @notice constructor
     * _LPFactory: LP factory
     * _SwapTwoPoolDeployer: Swap two pool deployer
     */
    constructor(
        IStableSwapLPFactory _LPFactory,
        IStableSwapDeployer _SwapTwoPoolDeployer
    ) {
        LPFactory = _LPFactory;
        SwapTwoPoolDeployer = _SwapTwoPoolDeployer;
    }

    // returns sorted token addresses, used to handle return values from pairs sorted in this order
    function sortTokens(
        address tokenA,
        address tokenB
    ) internal pure returns (address token0, address token1) {
        require(tokenA != tokenB, "IDENTICAL_ADDRESSES");
        (token0, token1) = tokenA < tokenB
            ? (tokenA, tokenB)
            : (tokenB, tokenA);
    }

    /**
     * @notice createSwapPair
     * @param _tokenA: Addresses of ERC20 conracts .
     * @param _tokenB: Addresses of ERC20 conracts .
     * @param _A: Amplification coefficient multiplied by n * (n - 1)
     * @param _fee: Fee to charge for exchanges
     * @param _admin_fee: Admin fee
     */
    function createSwapPair(
        address _tokenA,
        address _tokenB,
        uint256 _A,
        uint256 _fee,
        uint256 _admin_fee
    ) external onlyOwner {
        require(
            _tokenA != ZEROADDRESS &&
                _tokenB != ZEROADDRESS &&
                _tokenA != _tokenB,
            "Illegal token"
        );
        (address t0, address t1) = sortTokens(_tokenA, _tokenB);
        address LP = LPFactory.createSwapLP(t0, t1, address(this));
        address swapContract = SwapTwoPoolDeployer.createSwapPair(
            t0,
            t1,
            _A,
            _fee,
            _admin_fee,
            msg.sender,
            LP
        );

        IStableSwapLP(LP).setMinter(swapContract);
        emit NewStableSwapPair(swapContract, _tokenA, _tokenB, LP);
    }
}
