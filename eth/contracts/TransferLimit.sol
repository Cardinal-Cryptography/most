// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Ownable2StepUpgradeable} from "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";
import {AggregatorV3Interface} from "@chainlink/contracts/src/v0.8/shared/interfaces/AggregatorV3Interface.sol";
import {Math} from "@openzeppelin/contracts/utils/math/Math.sol";
import {ITransferLimit} from "./ITransferLimit.sol";

/// @title TransferLimit
/// @author Cardinal Cryptography
/// @notice Implements a transfer limit based on pricing data from Chainlink oracles
contract TransferLimit is
    ITransferLimit,
    UUPSUpgradeable,
    Ownable2StepUpgradeable
{
    mapping(address => uint256) public defaultMinima;

    struct USDMinimum {
        AggregatorV3Interface oracle;
        uint256 limit;
    }

    mapping(address => USDMinimum) public usdMinima;

    function initialize(address owner) public initializer {
        __Ownable_init(owner);
    }

    function _authorizeUpgrade(
        address newImplementation
    ) internal override onlyOwner {}

    /// @notice Set the default minimum transfer amount for a token - this is used if no USD oracle is set
    /// @param _token The token address
    /// @param _limit The minimum transfer amount
    function setDefaultLimit(address _token, uint256 _limit) public onlyOwner {
        defaultMinima[_token] = _limit;
    }

    /// @notice Set the USD oracle params for a token
    /// @param _token The token address
    /// @param _tokenDecimals The number of decimals the token has
    /// @param _oracle The Chainlink oracle address
    /// @param _limit The minimum transfer amount in USD with no decimals
    function setUSDOracle(
        address _token,
        uint256 _tokenDecimals,
        AggregatorV3Interface _oracle,
        uint256 _limit
    ) public onlyOwner {
        usdMinima[_token] = USDMinimum({
            oracle: _oracle,
            limit: _limit * 10 ** (_tokenDecimals + _oracle.decimals())
        });
    }

    /// @notice Get the minimum transfer amount for a given token based on the current configuration
    /// @param _token The token address
    function minimumTransferAmount(
        address _token
    ) public view returns (uint256) {
        uint256 minimum = defaultMinima[_token];

        USDMinimum memory config = usdMinima[_token];
        if (config.limit > 0) {
            (, int usdPrice, , , ) = config.oracle.latestRoundData();
            uint256 usdBasedMinimum = config.limit / uint256(usdPrice);

            minimum = Math.min(minimum, usdBasedMinimum);
        }

        return minimum;
    }

    /// @notice Check if a transfer of a given amount of a token is allowed - currently only checks if the minimum is met
    /// @param _token The token address
    /// @param _amount The amount of the token
    function isRequestAllowed(
        address _token,
        uint256 _amount
    ) public view returns (bool) {
        return _amount >= minimumTransferAmount(_token);
    }
}
