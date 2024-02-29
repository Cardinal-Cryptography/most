# Report


## Gas Optimizations


| |Issue|Instances|
|-|:-|:-:|
| [GAS-1](#GAS-1) | `a = a + b` is more gas effective than `a += b` for state variables (excluding arrays and mappings) | 1 |
| [GAS-2](#GAS-2) | Using bools for storage incurs overhead | 3 |
| [GAS-3](#GAS-3) | Cache array length outside of loop | 2 |
| [GAS-4](#GAS-4) | Use calldata instead of memory for function arguments that do not get mutated | 1 |
| [GAS-5](#GAS-5) | For Operations that will not overflow, you could use unchecked | 11 |
| [GAS-6](#GAS-6) | Use Custom Errors instead of Revert Strings to save Gas | 4 |
| [GAS-7](#GAS-7) | State variables only set in the constructor should be declared `immutable` | 1 |
| [GAS-8](#GAS-8) | Functions guaranteed to revert when called by normal users can be marked `payable` | 6 |
| [GAS-9](#GAS-9) | `++i` costs less gas compared to `i++` or `i += 1` (same for `--i` vs `i--` or `i -= 1`) | 5 |
| [GAS-10](#GAS-10) | Increments/decrements can be unchecked in for-loops | 2 |
| [GAS-11](#GAS-11) | Use != 0 instead of > 0 for unsigned integer comparison | 3 |
| [GAS-12](#GAS-12) | WETH address definition can be use directly | 1 |
### <a name="GAS-1"></a>[GAS-1] `a = a + b` is more gas effective than `a += b` for state variables (excluding arrays and mappings)
This saves **16 gas per instance.**

*Instances (1)*:
```solidity
File: Most.sol

273:         committeeId += 1;

```

### <a name="GAS-2"></a>[GAS-2] Using bools for storage incurs overhead
Use uint256(1) and uint256(2) for true/false to avoid a Gwarmaccess (100 gas), and to avoid Gsset (20000 gas) when changing from ‘false’ to ‘true’, after having been ‘true’ in the past. See [source](https://github.com/OpenZeppelin/openzeppelin-contracts/blob/58f635312aa21f947cae5f8578638a85aa2519f5/contracts/security/ReentrancyGuard.sol#L23-L27).

*Instances (3)*:
```solidity
File: Most.sol

23:         mapping(address => bool) signatures;

29:     mapping(bytes32 => bool) public processedRequests;

30:     mapping(bytes32 => bool) private committee;

```

### <a name="GAS-3"></a>[GAS-3] Cache array length outside of loop
If not cached, the solidity compiler will always read the length of the array during each iteration. That is, if it is a storage array, this is an extra sload operation (100 additional extra gas for each iteration except for the first) and if it is a memory array, this is an extra mload operation (3 additional gas for each iteration except for the first).

*Instances (2)*:
```solidity
File: Most.sol

76:         for (uint256 i = 0; i < _committee.length; i++) {

275:         for (uint256 i = 0; i < _committee.length; i++) {

```

### <a name="GAS-4"></a>[GAS-4] Use calldata instead of memory for function arguments that do not get mutated
When a function with a `memory` array is called externally, the `abi.decode()` step has to use a for-loop to copy each index of the `calldata` to the `memory` index. Each iteration of this for-loop costs at least 60 gas (i.e. `60 * <mem_array>.length`). Using `calldata` directly bypasses this loop. 

If the array is passed to an `internal` function which passes the array to another internal function where the array is modified and therefore `memory` is used in the `external` call, it's still more gas-efficient to use `calldata` when the `external` function uses modifiers, since the modifiers may prevent the internal functions from being called. Structs have the same overhead as an array of length one. 

 *Saves 60 gas per instance*

*Instances (1)*:
```solidity
File: Most.sol

261:         address[] memory _committee,

```

### <a name="GAS-5"></a>[GAS-5] For Operations that will not overflow, you could use unchecked

*Instances (11)*:
```solidity
File: Most.sol

5: import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

6: import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

7: import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

8: import "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";

9: import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

76:         for (uint256 i = 0; i < _committee.length; i++) {

128:         requestNonce++;

159:         requestNonce++;

197:         request.signatureCount++;

273:         committeeId += 1;

275:         for (uint256 i = 0; i < _committee.length; i++) {

```

### <a name="GAS-6"></a>[GAS-6] Use Custom Errors instead of Revert Strings to save Gas
Custom errors are available from solidity version 0.8.4. Custom errors save [**~50 gas**](https://gist.github.com/IllIllI000/ad1bd0d29a0101b25e57c293b4b0c746) each time they're hit by [avoiding having to allocate and store the revert string](https://blog.soliditylang.org/2021/04/21/custom-errors/#errors-in-depth). Not defining the strings also save deployment gas

Additionally, custom errors can be used inside and outside of contracts (including interfaces and libraries).

Source: <https://blog.soliditylang.org/2021/04/21/custom-errors/>:

> Starting from [Solidity v0.8.4](https://github.com/ethereum/solidity/releases/tag/v0.8.4), there is a convenient and gas-efficient way to explain to users why an operation failed through the use of custom errors. Until now, you could already use strings to give more information about failures (e.g., `revert("Insufficient funds.");`), but they are rather expensive, especially when it comes to deploy cost, and it is difficult to use dynamic information in them.

Consider replacing **all revert strings** with custom errors in the solution, and particularly those that have multiple occurrences:

*Instances (4)*:
```solidity
File: Most.sol

114:         require(destTokenAddress != 0x0, "Unsupported pair");

144:         require(destTokenAddress != 0x0, "Unsupported pair");

149:         require(success, "Failed deposit native ETH.");

194:         require(_requestHash == requestHash, "Hash does not match the data");

```

### <a name="GAS-7"></a>[GAS-7] State variables only set in the constructor should be declared `immutable`
Variables only set in the constructor and never edited afterwards should be marked as immutable, as it would avoid the expensive storage-writing operation in the constructor (around **20 000 gas** per variable) and replace the expensive storage-reading operations (around **2100 gas** per reading) to a less expensive value reading (**3 gas**)

*Instances (1)*:
```solidity
File: Migrations.sol

14:         owner = msg.sender;

```

### <a name="GAS-8"></a>[GAS-8] Functions guaranteed to revert when called by normal users can be marked `payable`
If a function modifier such as `onlyOwner` is used, the function will revert if a normal user tries to pay the function. Marking the function as `payable` will lower the gas cost for legitimate callers because the compiler will not include checks for whether a payment was provided.

*Instances (6)*:
```solidity
File: Most.sol

92:     function _authorizeUpgrade(address) internal override onlyOwner {}

95:     function renounceOwnership() public virtual override onlyOwner {}

230:     function pause() external onlyOwner {

234:     function unpause() external onlyOwner {

285:     function addPair(bytes32 from, bytes32 to) external onlyOwner {

289:     function removePair(bytes32 from) external onlyOwner {

```

### <a name="GAS-9"></a>[GAS-9] `++i` costs less gas compared to `i++` or `i += 1` (same for `--i` vs `i--` or `i -= 1`)
Pre-increments and pre-decrements are cheaper.

For a `uint256 i` variable, the following is true with the Optimizer enabled at 10k:

**Increment:**

- `i += 1` is the most expensive form
- `i++` costs 6 gas less than `i += 1`
- `++i` costs 5 gas less than `i++` (11 gas less than `i += 1`)

**Decrement:**

- `i -= 1` is the most expensive form
- `i--` costs 11 gas less than `i -= 1`
- `--i` costs 5 gas less than `i--` (16 gas less than `i -= 1`)

Note that post-increments (or post-decrements) return the old value before incrementing or decrementing, hence the name *post-increment*:

```solidity
uint i = 1;  
uint j = 2;
require(j == i++, "This will be false as i is incremented after the comparison");
```
  
However, pre-increments (or pre-decrements) return the new value:
  
```solidity
uint i = 1;  
uint j = 2;
require(j == ++i, "This will be true as i is incremented before the comparison");
```

In the pre-increment case, the compiler has to create a temporary variable (when used) for returning `1` instead of `2`.

Consider using pre-increments and pre-decrements where they are relevant (meaning: not where post-increments/decrements logic are relevant).

*Saves 5 gas per instance*

*Instances (5)*:
```solidity
File: Most.sol

76:         for (uint256 i = 0; i < _committee.length; i++) {

128:         requestNonce++;

159:         requestNonce++;

197:         request.signatureCount++;

275:         for (uint256 i = 0; i < _committee.length; i++) {

```

### <a name="GAS-10"></a>[GAS-10] Increments/decrements can be unchecked in for-loops
In Solidity 0.8+, there's a default overflow check on unsigned integers. It's possible to uncheck this in for-loops and save some gas at each iteration, but at the cost of some code readability, as this uncheck cannot be made inline.

[ethereum/solidity#10695](https://github.com/ethereum/solidity/issues/10695)

The change would be:

```diff
- for (uint256 i; i < numIterations; i++) {
+ for (uint256 i; i < numIterations;) {
 // ...  
+   unchecked { ++i; }
}  
```

These save around **25 gas saved** per instance.

The same can be applied with decrements (which should use `break` when `i == 0`).

The risk of overflow is non-existent for `uint256`.

*Instances (2)*:
```solidity
File: Most.sol

76:         for (uint256 i = 0; i < _committee.length; i++) {

275:         for (uint256 i = 0; i < _committee.length; i++) {

```

### <a name="GAS-11"></a>[GAS-11] Use != 0 instead of > 0 for unsigned integer comparison

*Instances (3)*:
```solidity
File: Migrations.sol

3: pragma solidity >=0.4.22 <0.9.0;

```

```solidity
File: Most.sol

66:             _signatureThreshold > 0,

265:             _signatureThreshold > 0,

```

### <a name="GAS-12"></a>[GAS-12] WETH address definition can be use directly
WETH is a wrap Ether contract with a specific address in the Ethereum network, giving the option to define it may cause false recognition, it is healthier to define it directly.

    Advantages of defining a specific contract directly:
    
    It saves gas,
    Prevents incorrect argument definition,
    Prevents execution on a different chain and re-signature issues,
    WETH Address : 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2

*Instances (1)*:
```solidity
File: Most.sol

19:     address payable public wethAddress;

```


## Non Critical Issues


| |Issue|Instances|
|-|:-|:-:|
| [NC-1](#NC-1) | Replace `abi.encodeWithSignature` and `abi.encodeWithSelector` with `abi.encodeCall` which keeps the code typo/type safe | 2 |
| [NC-2](#NC-2) | Use `string.concat()` or `bytes.concat()` instead of `abi.encodePacked` | 4 |
| [NC-3](#NC-3) | Control structures do not follow the Solidity Style Guide | 3 |
| [NC-4](#NC-4) | Duplicated `require()`/`revert()` Checks Should Be Refactored To A Modifier Or Function | 6 |
| [NC-5](#NC-5) | Event missing indexed field | 4 |
| [NC-6](#NC-6) | Function ordering does not follow the Solidity style guide | 1 |
| [NC-7](#NC-7) | Functions should not be longer than 50 lines | 9 |
| [NC-8](#NC-8) | Change uint to uint256 | 1 |
| [NC-9](#NC-9) | Lack of checks in setters | 1 |
| [NC-10](#NC-10) | Missing Event for critical parameters change | 2 |
| [NC-11](#NC-11) | NatSpec is completely non-existent on functions that should have them | 11 |
| [NC-12](#NC-12) | Use a `modifier` instead of a `require/if` statement for a special `msg.sender` actor | 2 |
| [NC-13](#NC-13) | Consider using named mappings | 7 |
| [NC-14](#NC-14) | Owner can renounce while system is paused | 2 |
| [NC-15](#NC-15) | Contract does not follow the Solidity style guide's suggested layout ordering | 1 |
| [NC-16](#NC-16) | Internal and private variables and functions names should begin with an underscore | 3 |
| [NC-17](#NC-17) | Event is missing `indexed` fields | 4 |
| [NC-18](#NC-18) | `public` functions not called by the contract should be declared `external` instead | 3 |
| [NC-19](#NC-19) | Variables need not be initialized to zero | 2 |
### <a name="NC-1"></a>[NC-1] Replace `abi.encodeWithSignature` and `abi.encodeWithSelector` with `abi.encodeCall` which keeps the code typo/type safe
When using `abi.encodeWithSignature`, it is possible to include a typo for the correct function signature.
When using `abi.encodeWithSignature` or `abi.encodeWithSelector`, it is also possible to provide parameters that are not of the correct type for the function.

To avoid these pitfalls, it would be best to use [`abi.encodeCall`](https://solidity-by-example.org/abi-encode/) instead.

*Instances (2)*:
```solidity
File: Most.sol

147:             abi.encodeWithSignature("deposit()")

208:                     abi.encodeWithSignature("withdraw(uint256)", amount)

```

### <a name="NC-2"></a>[NC-2] Use `string.concat()` or `bytes.concat()` instead of `abi.encodePacked`
Solidity version 0.8.4 introduces `bytes.concat()` (vs `abi.encodePacked(<bytes>,<bytes>)`)

Solidity version 0.8.12 introduces `string.concat()` (vs `abi.encodePacked(<str>,<str>), which catches concatenation errors (in the event of a `bytes` data mixed in the concatenation)`)

*Instances (4)*:
```solidity
File: Most.sol

78:                 keccak256(abi.encodePacked(committeeId, _committee[i]))

179:             abi.encodePacked(

249:         return committee[keccak256(abi.encodePacked(_committeeId, account))];

277:                 keccak256(abi.encodePacked(committeeId, _committee[i]))

```

### <a name="NC-3"></a>[NC-3] Control structures do not follow the Solidity Style Guide
See the [control structures](https://docs.soliditylang.org/en/latest/style-guide.html#control-structures) section of the Solidity Style Guide

*Instances (3)*:
```solidity
File: Migrations.sol

10:         if (msg.sender == owner) _;

```

```solidity
File: Most.sol

71:             "Not enough guardians specified"

270:             "Not enough guardians specified"

```

### <a name="NC-4"></a>[NC-4] Duplicated `require()`/`revert()` Checks Should Be Refactored To A Modifier Or Function

*Instances (6)*:
```solidity
File: Most.sol

65:         require(

69:         require(

114:         require(destTokenAddress != 0x0, "Unsupported pair");

144:         require(destTokenAddress != 0x0, "Unsupported pair");

264:         require(

268:         require(

```

### <a name="NC-5"></a>[NC-5] Event missing indexed field
Index event fields make the field more quickly accessible [to off-chain tools](https://ethereum.stackexchange.com/questions/40396/can-somebody-please-explain-the-concept-of-event-indexing) that parse events. This is especially useful when it comes to filtering based on an address. However, note that each index field costs extra gas during emission, so it's not necessarily best to index the maximum allowed per event (three fields). Where applicable, each `event` should use three `indexed` fields if there are three or more fields, and gas usage is not particularly of concern for the events in question. If there are fewer than three applicable fields, all of the applicable fields should be indexed.

*Instances (4)*:
```solidity
File: Most.sol

42:     event RequestSigned(bytes32 requestHash, address signer);

44:     event RequestProcessed(bytes32 requestHash);

47:     event ProcessedRequestSigned(bytes32 requestHash, address signer);

49:     event RequestAlreadySigned(bytes32 requestHash, address signer);

```

### <a name="NC-6"></a>[NC-6] Function ordering does not follow the Solidity style guide
According to the [Solidity style guide](https://docs.soliditylang.org/en/v0.8.17/style-guide.html#order-of-functions), functions should be laid out in the following order :`constructor()`, `receive()`, `fallback()`, `external`, `public`, `internal`, `private`, but the cases below do not follow this pattern

*Instances (1)*:
```solidity
File: Most.sol

1: 
   Current order:
   public initialize
   internal _authorizeUpgrade
   public renounceOwnership
   external sendRequest
   external sendRequestNative
   external receiveRequest
   external pause
   external unpause
   external hasSignedRequest
   public isInCommittee
   internal bytes32ToAddress
   internal addressToBytes32
   external setCommittee
   external addPair
   external removePair
   
   Suggested order:
   external sendRequest
   external sendRequestNative
   external receiveRequest
   external pause
   external unpause
   external hasSignedRequest
   external setCommittee
   external addPair
   external removePair
   public initialize
   public renounceOwnership
   public isInCommittee
   internal _authorizeUpgrade
   internal bytes32ToAddress
   internal addressToBytes32

```

### <a name="NC-7"></a>[NC-7] Functions should not be longer than 50 lines
Overly complex code can make understanding functionality more difficult, try to further modularize your code to ensure readability 

*Instances (9)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {

21:     function upgrade(address new_address) public restricted {

```

```solidity
File: Most.sol

92:     function _authorizeUpgrade(address) internal override onlyOwner {}

95:     function renounceOwnership() public virtual override onlyOwner {}

137:     function sendRequestNative(bytes32 destReceiverAddress) external payable {

252:     function bytes32ToAddress(bytes32 data) internal pure returns (address) {

256:     function addressToBytes32(address addr) internal pure returns (bytes32) {

285:     function addPair(bytes32 from, bytes32 to) external onlyOwner {

289:     function removePair(bytes32 from) external onlyOwner {

```

### <a name="NC-8"></a>[NC-8] Change uint to uint256
Throughout the code base, some variables are declared as `uint`. To favor explicitness, consider changing all instances of `uint` to `uint256`

*Instances (1)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {

```

### <a name="NC-9"></a>[NC-9] Lack of checks in setters
Be it sanity checks (like checks against `0`-values) or initial setting checks: it's best for Setter functions to have them

*Instances (1)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {
            last_completed_migration = completed;

```

### <a name="NC-10"></a>[NC-10] Missing Event for critical parameters change
Events help non-contract tools to track changes, and events prevent users from being surprised by changes.

*Instances (2)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {
            last_completed_migration = completed;

```

```solidity
File: Most.sol

260:     function setCommittee(
             address[] memory _committee,
             uint256 _signatureThreshold
         ) external onlyOwner {
             require(
                 _signatureThreshold > 0,
                 "Signature threshold must be greater than 0"
             );
             require(
                 _committee.length >= _signatureThreshold,
                 "Not enough guardians specified"
             );
     
             committeeId += 1;
     
             for (uint256 i = 0; i < _committee.length; i++) {
                 committee[
                     keccak256(abi.encodePacked(committeeId, _committee[i]))
                 ] = true;
             }
     
             committeeSize[committeeId] = _committee.length;
             signatureThreshold[committeeId] = _signatureThreshold;

```

### <a name="NC-11"></a>[NC-11] NatSpec is completely non-existent on functions that should have them
Public and external functions that aren't view or pure should have NatSpec comments

*Instances (11)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {

21:     function upgrade(address new_address) public restricted {

```

```solidity
File: Most.sol

59:     function initialize(

103:     function sendRequest(

137:     function sendRequestNative(bytes32 destReceiverAddress) external payable {

163:     function receiveRequest(

230:     function pause() external onlyOwner {

234:     function unpause() external onlyOwner {

260:     function setCommittee(

285:     function addPair(bytes32 from, bytes32 to) external onlyOwner {

289:     function removePair(bytes32 from) external onlyOwner {

```

### <a name="NC-12"></a>[NC-12] Use a `modifier` instead of a `require/if` statement for a special `msg.sender` actor
If a function is supposed to be access-controlled, a `modifier` should be used instead of a `require/if` statement for more readability.

*Instances (2)*:
```solidity
File: Migrations.sol

10:         if (msg.sender == owner) _;

```

```solidity
File: Most.sol

189:         if (request.signatures[msg.sender]) {

```

### <a name="NC-13"></a>[NC-13] Consider using named mappings
Consider moving to solidity version 0.8.18 or later, and using [named mappings](https://ethereum.stackexchange.com/questions/51629/how-to-name-the-arguments-in-mapping/145555#145555) to make it easier to understand the purpose of each mapping

*Instances (7)*:
```solidity
File: Most.sol

23:         mapping(address => bool) signatures;

27:     mapping(bytes32 => bytes32) public supportedPairs;

28:     mapping(bytes32 => Request) public pendingRequests;

29:     mapping(bytes32 => bool) public processedRequests;

30:     mapping(bytes32 => bool) private committee;

31:     mapping(uint256 => uint256) public committeeSize;

32:     mapping(uint256 => uint256) public signatureThreshold;

```

### <a name="NC-14"></a>[NC-14] Owner can renounce while system is paused
The contract owner or single user with a role is not prevented from renouncing the role/ownership while the contract is paused, which would cause any user assets stored in the protocol, to be locked indefinitely.

*Instances (2)*:
```solidity
File: Most.sol

230:     function pause() external onlyOwner {

234:     function unpause() external onlyOwner {

```

### <a name="NC-15"></a>[NC-15] Contract does not follow the Solidity style guide's suggested layout ordering
The [style guide](https://docs.soliditylang.org/en/v0.8.16/style-guide.html#order-of-layout) says that, within a contract, the ordering should be:

1) Type declarations
2) State variables
3) Events
4) Modifiers
5) Functions

However, the contract(s) below do not follow this ordering

*Instances (1)*:
```solidity
File: Most.sol

1: 
   Current order:
   VariableDeclaration.requestNonce
   VariableDeclaration.committeeId
   VariableDeclaration.wethAddress
   StructDefinition.Request
   VariableDeclaration.supportedPairs
   VariableDeclaration.pendingRequests
   VariableDeclaration.processedRequests
   VariableDeclaration.committee
   VariableDeclaration.committeeSize
   VariableDeclaration.signatureThreshold
   EventDefinition.CrosschainTransferRequest
   EventDefinition.RequestSigned
   EventDefinition.RequestProcessed
   EventDefinition.ProcessedRequestSigned
   EventDefinition.RequestAlreadySigned
   ModifierDefinition._onlyCommitteeMember
   FunctionDefinition.initialize
   FunctionDefinition._authorizeUpgrade
   FunctionDefinition.renounceOwnership
   FunctionDefinition.sendRequest
   FunctionDefinition.sendRequestNative
   FunctionDefinition.receiveRequest
   FunctionDefinition.pause
   FunctionDefinition.unpause
   FunctionDefinition.hasSignedRequest
   FunctionDefinition.isInCommittee
   FunctionDefinition.bytes32ToAddress
   FunctionDefinition.addressToBytes32
   FunctionDefinition.setCommittee
   FunctionDefinition.addPair
   FunctionDefinition.removePair
   FunctionDefinition.receive
   
   Suggested order:
   VariableDeclaration.requestNonce
   VariableDeclaration.committeeId
   VariableDeclaration.wethAddress
   VariableDeclaration.supportedPairs
   VariableDeclaration.pendingRequests
   VariableDeclaration.processedRequests
   VariableDeclaration.committee
   VariableDeclaration.committeeSize
   VariableDeclaration.signatureThreshold
   StructDefinition.Request
   EventDefinition.CrosschainTransferRequest
   EventDefinition.RequestSigned
   EventDefinition.RequestProcessed
   EventDefinition.ProcessedRequestSigned
   EventDefinition.RequestAlreadySigned
   ModifierDefinition._onlyCommitteeMember
   FunctionDefinition.initialize
   FunctionDefinition._authorizeUpgrade
   FunctionDefinition.renounceOwnership
   FunctionDefinition.sendRequest
   FunctionDefinition.sendRequestNative
   FunctionDefinition.receiveRequest
   FunctionDefinition.pause
   FunctionDefinition.unpause
   FunctionDefinition.hasSignedRequest
   FunctionDefinition.isInCommittee
   FunctionDefinition.bytes32ToAddress
   FunctionDefinition.addressToBytes32
   FunctionDefinition.setCommittee
   FunctionDefinition.addPair
   FunctionDefinition.removePair
   FunctionDefinition.receive

```

### <a name="NC-16"></a>[NC-16] Internal and private variables and functions names should begin with an underscore
According to the Solidity Style Guide, Non-`external` variable and function names should begin with an [underscore](https://docs.soliditylang.org/en/latest/style-guide.html#underscore-prefix-for-non-external-functions-and-variables)

*Instances (3)*:
```solidity
File: Most.sol

30:     mapping(bytes32 => bool) private committee;

252:     function bytes32ToAddress(bytes32 data) internal pure returns (address) {

256:     function addressToBytes32(address addr) internal pure returns (bytes32) {

```

### <a name="NC-17"></a>[NC-17] Event is missing `indexed` fields
Index event fields make the field more quickly accessible to off-chain tools that parse events. However, note that each index field costs extra gas during emission, so it's not necessarily best to index the maximum allowed per event (three fields). Each event should use three indexed fields if there are three or more fields, and gas usage is not particularly of concern for the events in question. If there are fewer than three fields, all of the fields should be indexed.

*Instances (4)*:
```solidity
File: Most.sol

42:     event RequestSigned(bytes32 requestHash, address signer);

44:     event RequestProcessed(bytes32 requestHash);

47:     event ProcessedRequestSigned(bytes32 requestHash, address signer);

49:     event RequestAlreadySigned(bytes32 requestHash, address signer);

```

### <a name="NC-18"></a>[NC-18] `public` functions not called by the contract should be declared `external` instead

*Instances (3)*:
```solidity
File: Migrations.sol

17:     function setCompleted(uint completed) public restricted {

21:     function upgrade(address new_address) public restricted {

```

```solidity
File: Most.sol

59:     function initialize(

```

### <a name="NC-19"></a>[NC-19] Variables need not be initialized to zero
The default value for variables is zero, so initializing them to zero is superfluous.

*Instances (2)*:
```solidity
File: Most.sol

76:         for (uint256 i = 0; i < _committee.length; i++) {

275:         for (uint256 i = 0; i < _committee.length; i++) {

```


## Low Issues


| |Issue|Instances|
|-|:-|:-:|
| [L-1](#L-1) | Some tokens may revert when zero value transfers are made | 2 |
| [L-2](#L-2) | Empty Function Body - Consider commenting why | 1 |
| [L-3](#L-3) | Empty `receive()/payable fallback()` function does not authenticate requests | 1 |
| [L-4](#L-4) | External call recipient may consume all transaction gas | 3 |
| [L-5](#L-5) | Initializers could be front-run | 4 |
| [L-6](#L-6) | Owner can renounce while system is paused | 2 |
| [L-7](#L-7) | Solidity version 0.8.20+ may not work on other chains due to `PUSH0` | 1 |
| [L-8](#L-8) | Consider using OpenZeppelin's SafeCast library to prevent unexpected overflows when downcasting | 1 |
| [L-9](#L-9) | Unsafe ERC20 operation(s) | 2 |
| [L-10](#L-10) | Unspecific compiler version pragma | 1 |
| [L-11](#L-11) | Upgradeable contract is missing a `__gap[50]` storage variable to allow for new storage variables in later versions | 7 |
| [L-12](#L-12) | Upgradeable contract not initialized | 11 |
### <a name="L-1"></a>[L-1] Some tokens may revert when zero value transfers are made
Example: https://github.com/d-xo/weird-erc20#revert-on-zero-value-transfers.

In spite of the fact that EIP-20 [states](https://github.com/ethereum/EIPs/blob/46b9b698815abbfa628cd1097311deee77dd45c5/EIPS/eip-20.md?plain=1#L116) that zero-valued transfers must be accepted, some tokens, such as LEND will revert if this is attempted, which may cause transactions that involve other tokens (such as batch operations) to fully revert. Consider skipping the transfer if the amount is zero, which will also save gas.

*Instances (2)*:
```solidity
File: Most.sol

118:         token.transferFrom(sender, address(this), amount);

224:                 token.transfer(bytes32ToAddress(destReceiverAddress), amount);

```

### <a name="L-2"></a>[L-2] Empty Function Body - Consider commenting why

*Instances (1)*:
```solidity
File: Most.sol

92:     function _authorizeUpgrade(address) internal override onlyOwner {}

```

### <a name="L-3"></a>[L-3] Empty `receive()/payable fallback()` function does not authenticate requests
If the intention is for the Ether to be used, the function should call another function, otherwise it should revert (e.g. require(msg.sender == address(weth))). Having no access control on the function means that someone may send Ether to the contract, and have no way to get anything back out, which is a loss of funds. If the concern is having to spend a small amount of gas to check the sender against an immutable address, the code should at least have a function to rescue unused Ether.

*Instances (1)*:
```solidity
File: Most.sol

293:     receive() external payable {}

```

### <a name="L-4"></a>[L-4] External call recipient may consume all transaction gas
There is no limit specified on the amount of gas used, so the recipient can use up all of the transaction's gas, causing it to revert. Use `addr.call{gas: <amount>}("")` or [this](https://github.com/nomad-xyz/ExcessivelySafeCall) library instead.

*Instances (3)*:
```solidity
File: Most.sol

146:         (bool success, ) = wethAddress.call{value: amount}(

207:                 (bool unwrapSuccess, ) = wethAddress.call(

216:                 ).call{value: amount}("");

```

### <a name="L-5"></a>[L-5] Initializers could be front-run
Initializers could be front-run, allowing an attacker to either set their own values, take ownership of the contract, and in the best case forcing a re-deployment

*Instances (4)*:
```solidity
File: Most.sol

59:     function initialize(

64:     ) public initializer {

86:         __Ownable_init(owner);

88:         __Pausable_init();

```

### <a name="L-6"></a>[L-6] Owner can renounce while system is paused
The contract owner or single user with a role is not prevented from renouncing the role/ownership while the contract is paused, which would cause any user assets stored in the protocol, to be locked indefinitely.

*Instances (2)*:
```solidity
File: Most.sol

230:     function pause() external onlyOwner {

234:     function unpause() external onlyOwner {

```

### <a name="L-7"></a>[L-7] Solidity version 0.8.20+ may not work on other chains due to `PUSH0`
The compiler for Solidity 0.8.20 switches the default target EVM version to [Shanghai](https://blog.soliditylang.org/2023/05/10/solidity-0.8.20-release-announcement/#important-note), which includes the new `PUSH0` op code. This op code may not yet be implemented on all L2s, so deployment on these chains will fail. To work around this issue, use an earlier [EVM](https://docs.soliditylang.org/en/v0.8.20/using-the-compiler.html?ref=zaryabs.com#setting-the-evm-version-to-target) [version](https://book.getfoundry.sh/reference/config/solidity-compiler#evm_version). While the project itself may or may not compile with 0.8.20, other projects with which it integrates, or which extend this project may, and those projects will have problems deploying these contracts/libraries.

*Instances (1)*:
```solidity
File: Most.sol

3: pragma solidity ^0.8.20;

```

### <a name="L-8"></a>[L-8] Consider using OpenZeppelin's SafeCast library to prevent unexpected overflows when downcasting
Downcasting from `uint256`/`int256` in Solidity does not revert on overflow. This can result in undesired exploitation or bugs, since developers usually assume that overflows raise errors. [OpenZeppelin's SafeCast library](https://docs.openzeppelin.com/contracts/3.x/api/utils#SafeCast) restores this intuition by reverting the transaction when such an operation overflows. Using this library eliminates an entire class of bugs, so it's recommended to use it always. Some exceptions are acceptable like with the classic `uint256(uint160(address(variable)))`

*Instances (1)*:
```solidity
File: Most.sol

257:         return bytes32(uint256(uint160(addr)));

```

### <a name="L-9"></a>[L-9] Unsafe ERC20 operation(s)

*Instances (2)*:
```solidity
File: Most.sol

118:         token.transferFrom(sender, address(this), amount);

224:                 token.transfer(bytes32ToAddress(destReceiverAddress), amount);

```

### <a name="L-10"></a>[L-10] Unspecific compiler version pragma

*Instances (1)*:
```solidity
File: Migrations.sol

3: pragma solidity >=0.4.22 <0.9.0;

```

### <a name="L-11"></a>[L-11] Upgradeable contract is missing a `__gap[50]` storage variable to allow for new storage variables in later versions
See [this](https://docs.openzeppelin.com/contracts/4.x/upgradeable#storage_gaps) link for a description of this storage variable. While some contracts may not currently be sub-classed, adding the variable now protects against forgetting to add it in the future.

*Instances (7)*:
```solidity
File: Most.sol

6: import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

7: import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

8: import "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";

9: import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

13:     UUPSUpgradeable,

14:     Ownable2StepUpgradeable,

15:     PausableUpgradeable

```

### <a name="L-12"></a>[L-12] Upgradeable contract not initialized
Upgradeable contracts are initialized via an initializer function rather than by a constructor. Leaving such a contract uninitialized may lead to it being taken over by a malicious user

*Instances (11)*:
```solidity
File: Most.sol

6: import "@openzeppelin/contracts-upgradeable/proxy/utils/Initializable.sol";

7: import "@openzeppelin/contracts-upgradeable/proxy/utils/UUPSUpgradeable.sol";

8: import "@openzeppelin/contracts-upgradeable/access/Ownable2StepUpgradeable.sol";

9: import "@openzeppelin/contracts-upgradeable/utils/PausableUpgradeable.sol";

13:     UUPSUpgradeable,

14:     Ownable2StepUpgradeable,

15:     PausableUpgradeable

59:     function initialize(

64:     ) public initializer {

86:         __Ownable_init(owner);

88:         __Pausable_init();

```


## Medium Issues


| |Issue|Instances|
|-|:-|:-:|
| [M-1](#M-1) | Contracts are vulnerable to fee-on-transfer accounting-related issues | 1 |
| [M-2](#M-2) | Centralization Risk for trusted owners | 7 |
| [M-3](#M-3) | Return values of `transfer()`/`transferFrom()` not checked | 2 |
| [M-4](#M-4) | Unsafe use of `transfer()`/`transferFrom()` with `IERC20` | 2 |
### <a name="M-1"></a>[M-1] Contracts are vulnerable to fee-on-transfer accounting-related issues
Consistently check account balance before and after transfers for Fee-On-Transfer discrepancies. As arbitrary ERC20 tokens can be used, the amount here should be calculated every time to take into consideration a possible fee-on-transfer or deflation.
Also, it's a good practice for the future of the solution.

Use the balance before and after the transfer to calculate the received amount instead of assuming that it would be equal to the amount passed as a parameter. Or explicitly document that such tokens shouldn't be used and won't be supported

*Instances (1)*:
```solidity
File: Most.sol

118:         token.transferFrom(sender, address(this), amount);

```

### <a name="M-2"></a>[M-2] Centralization Risk for trusted owners

#### Impact:
Contracts have owners with privileged rights to perform admin tasks and need to be trusted to not perform malicious updates or drain funds.

*Instances (7)*:
```solidity
File: Most.sol

92:     function _authorizeUpgrade(address) internal override onlyOwner {}

95:     function renounceOwnership() public virtual override onlyOwner {}

230:     function pause() external onlyOwner {

234:     function unpause() external onlyOwner {

263:     ) external onlyOwner {

285:     function addPair(bytes32 from, bytes32 to) external onlyOwner {

289:     function removePair(bytes32 from) external onlyOwner {

```

### <a name="M-3"></a>[M-3] Return values of `transfer()`/`transferFrom()` not checked
Not all `IERC20` implementations `revert()` when there's a failure in `transfer()`/`transferFrom()`. The function signature has a `boolean` return value and they indicate errors that way instead. By not checking the return value, operations that should have marked as failed, may potentially go through without actually making a payment

*Instances (2)*:
```solidity
File: Most.sol

118:         token.transferFrom(sender, address(this), amount);

224:                 token.transfer(bytes32ToAddress(destReceiverAddress), amount);

```

### <a name="M-4"></a>[M-4] Unsafe use of `transfer()`/`transferFrom()` with `IERC20`
Some tokens do not implement the ERC20 standard properly but are still accepted by most code that accepts ERC20 tokens.  For example Tether (USDT)'s `transfer()` and `transferFrom()` functions on L1 do not return booleans as the specification requires, and instead have no return value. When these sorts of tokens are cast to `IERC20`, their [function signatures](https://medium.com/coinmonks/missing-return-value-bug-at-least-130-tokens-affected-d67bf08521ca) do not match and therefore the calls made, revert (see [this](https://gist.github.com/IllIllI000/2b00a32e8f0559e8f386ea4f1800abc5) link for a test case). Use OpenZeppelin's `SafeERC20`'s `safeTransfer()`/`safeTransferFrom()` instead

*Instances (2)*:
```solidity
File: Most.sol

118:         token.transferFrom(sender, address(this), amount);

224:                 token.transfer(bytes32ToAddress(destReceiverAddress), amount);

```

