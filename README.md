# Encointer Pallets

This branch contains the deprecated `pallet-personhood-oracle` and `pallet-sybil-gate-template`. The demo intended to 
demonstrate how a parachain can integrate the `pallet-sybil-gate-template` to query an accounts `personhood` confidence
level from the Encointer Parachain via the `pallet-personhood-oracle`. However, this demo is deemed deprecated, as
we assume that this can be done without the XCM-protocol in the future, which is way too powerful and complicated
for a simple cross-chain storage query. We assume that beefy, will be used in the future to formulate a simple
storage proof of another parachain, which does not need any XCM at all.

[![Rust](https://github.com/encointer/pallets/actions/workflows/ci.yml/badge.svg)](https://github.com/encointer/pallets/actions/workflows/ci.yml)

All application-specific pallets for [encointer](https://encointer.org)

## pallet-encointer-ceremonies
a substrate pallet to perform encointer ceremonies

## pallet-encointer-communities
A substrate pallet for encointer communities and managing their meetup locations

## pallet-encointer-ceremonies
a substrate pallet to perform encointer ceremonies

## pallet-encointer-balances
a balances module that supports multiple communities and demurrage

## pallet-encointer-bazaar
a registry for classifieds from community members, linking to IPFS

## personhood-oracle
a digital personhood verification oracle with XCM support. See pallet sybil-gate-example for how to use this from another parachain

## Dev Hints

### Benchmarking
You can automatically update the `WeightInfo` definitions by running the benchmarks in an encointer-node with the
script in the node's repository: `./scripts/benchmark_runtime.sh` and uncommenting the line with 
`frame-weight-template-full-info.hbs` (see the script's documentation).

### Serializing
* There is a know issue with serializing u-/i128 in the json-rpc crate, see (https://github.com/paritytech/substrate/issues/4641). 
This affects us predominantly when serializing fixed point numbers in the custom RPCs. There is a custom serialization
shim as a workaround for that issue in [ep-core](./primitives/core), which can be used as custom serde attribute like:

```rust
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct BalanceEntry<BlockNumber> {
	/// The balance of the account after last manual adjustment
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_fixed"))]
	pub principal: BalanceType,
	/// The time (block height) at which the balance was last adjusted
	pub last_update: BlockNumber,
}
```

We also apply substrate's standard of serializing with `camelCase`.

### Versioning

We bump crate versions separately and tag the repository with the highest crate version

motivation: git blame should show on crate directory level if there was a change. This way, browsing the repo on github really shows when a certain pallet or crate has been touched. Even if it's only adjustments for upstream upgrades, just bump crate versions to the newest, which will be tagged globally

Pallet repo version does not need to be aligned with neither node or parachain (or runtime) crate versions - although this has been the case in the past.