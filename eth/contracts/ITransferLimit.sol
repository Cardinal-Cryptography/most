// SPDX-License-Identifier: MIT

pragma solidity ^0.8.20;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

interface ITransferLimit {
    function isRequestAllowed(
        address _token,
        uint256 _amount
    ) external view returns (bool);
}
