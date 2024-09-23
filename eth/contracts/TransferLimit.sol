// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {UUPSUpgradeable} from "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";
import {Ownable2StepUpgradeable} from "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";
import {AggregatorV3Interface} from "@chainlink/contracts/src/v0.8/shared/interfaces/AggregatorV3Interface.sol";
import {Math} from "@openzeppelin/contracts/utils/math/Math.sol";
import {ITransferLimit} from "./ITransferLimit.sol";

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

    function _authorizeUpgrade(
        address newImplementation
    ) internal override onlyOwner {}

    function setDefaultLimit(address _token, uint256 _limit) public onlyOwner {
        defaultMinima[_token] = _limit;
    }

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

    function isRequestAllowed(
        address _token,
        uint256 _amount
    ) public view returns (bool) {
        return _amount >= minimumTransferAmount(_token);
    }
}
