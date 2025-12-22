// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {ZKRollupBridge} from "../bridge/ZKRollupBridge.sol";

contract TestZKRollupBridge is ZKRollupBridge {
    constructor(address _verifier, bytes32 _genesisRoot) ZKRollupBridge(_verifier, _genesisRoot) {}
}
