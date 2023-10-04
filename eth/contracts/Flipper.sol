pragma solidity >=0.4.22 <0.9.0;

contract Flipper {
    bool public flipValue;
    bool public flopValue;

    event Flip(bool value);
    event Flop(bool value);

    constructor() public {
        flipValue = false;
        flopValue = false;
    }

    function flip() public {
        flipValue = !flipValue;
        emit Flip(flipValue);
    }

    function flop() public {
        flopValue = !flopValue;
        emit Flop(flopValue);
    }
}
