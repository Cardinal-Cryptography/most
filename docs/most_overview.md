# MOST overview

This document describes the overall architecture of MOST, its various components, and their function.

## High-level view

MOST is a custodial bridge between Ethereum and Aleph Zero. It works by minting tokens on one chain that represent some token on the other chain. The invariant it maintains is that for any amount of wrapped tokens minted, the same amount of source tokens are kept in the contract on the other side of the bridge. For example, if there are a total of 1234 Wrapped Ether minted on the Aleph Zero side, then 1234 Wrapped Ether remain owned by the contract on the Ethereum side.

The way the invariant is maintained is by:

1. Whenever someone transfers native tokens into the bridge, we mint the matching token in the same quantity for them on the other chain.
2. Whenever someone transfers bridge tokens to the bridge, we burn those tokens and release the same amount to them on the other chain.

At the time of this writing, the bridge only handles native tokens on the Ethereum side and any minting and burning happens on the Aleph Zero side. However, work is ongoing to change that and allow Aleph Zero tokens to be bridged onto Ethereum.

The bridge is custodial in that the owner of the bridge (or rather a comittee of owners, see [Guardians](#guardians)) guarantees with their signature that the invariant described above is being maintained. Conceptually, the owner observes both chains and performs actions as needed to maintain the invariant, and, in fact, transfer any bridged assets to the correct target account.

## Guardians

The owner of the bridge is not a single entity, but rather a number of owners who vote on any actions with a simple majority. At the time of this writing there are 8 guardians and 5 votes are needed for any actions. Each guardian is in fact represented by 2 public addresses - a governance address and a signing address. Theoretically, the governance committee and the signing committee could have different membership (in term of business entities/persons, they _are_ different in terms of the actual keys), but that's not the case right now.

In practice, a signijwng key is generated during the setup of a Guardian's infrastructure. These keys are then submitted to the governance committee. Finally, the governance committee votes to set the signing committee to the set of keys received in this manner.

### Governance committee

The governance committee votes on any major actions that change the bridge operations. Some of the most important ones are:

- Stopping and starting the bridge
- Changing bridge configuration paramters, including changing committee memberships
- Updating the code of the bridge contracts

The governance committee is implemented as a multisig address on the Aleph Zero side and as a Gnosis Safe account on the Aleph Zero side.

### Signing committee

The signing committee basically performs only two types of operations. Both essentially comprise in signing bridging requests, that is certifying on one chain that something happened on the other. The first case is certifying that a certain amount of some native token has been transferred into the bridge contract on one chain, which triggers a minting of the same amount of the relevant token on the other side. The second case, is certifying that a certain amount of a bridge token has been transferred into the bridge contract on one chain and burned, which triggers the release of the same amount of tokens from the bridge contract on the other side.

## Main components

### Contracts

#### Most.sol (Ethereum) / Most (Aleph Zero)

This pair of contracts are the main entrypoints to the bridge (if you ignore the frontend). Bridging is triggered by transferring tokens into these contracts. Note that it technically happens as a `send_request` contract call, with the transfer happening as part of that call.

#### Token (Aleph Zero)

This contract is a PSP22 implementation with minting and burning enabled by a designated role. The bridge contract is given minter/burner privileges, which it uses when processing requests (see [High-level view](#high-level-view)).

### Relayer

The relayer is the software component which perform the actual signing as described in [Signing committee](#signing-committee). See `docs/relayer_hldd.md` for more details on its architecture.

## Supporting components

### Signer

The signer is a separate component that keeps a pair of signing keys (one Ethereum and one Aleph Zero key) and communicates with the relayer over VSOCK to sign any transaction that is submitted to it with the appropriate key. This component is intended to be deployed in an AWS nitro enclave to enhance key security.

### Gas Price Oracle (Aleph Zero)

This contract delivers information about Ethereum gas prices to the main Most contracts on the Aleph Zero side. This is used in order to charge the appropriate amount in fees when accepting requests for Aleph Zero -> Ethereum transfers. The contract is intended to be fed with data by an external feeder, source not included in this repo. The main contract has fallback pricing setup so that it works even in the absence of the Gas Price Oracle or when it starts to provide malicious data.

## Frontend

The frontend for this project consists in just one tab of the overall Common frontend. As such it is contained in the [Common frontend](https://github.com/Cardinal-Cryptography/common-frontend/) repo.

## Vocabulary

- bridge token - a token managed by the bridge that is minted and burned in response to bridge requests
- native token - any token that is not a bridge token, can either be the actual native token of the chain (Ether or Azero) or an ERC20/PSP22 smart contract token
