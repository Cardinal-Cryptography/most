pragma solidity >=0.4.22 <0.9.0;

contract Flipper {
  bool public value;

  event Flip(bool newValue);

  constructor() public {
    value = false;
  }

  function flip() public {
    value = !value;
    emit Flip(value);
  }
}
