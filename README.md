# Encointer Pallets

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

## ~~personhood-oracle & sybil-gate template~~ [Deprecated]
A digital personhood verification oracle with XCM support. See the README.md on the stale demo branch for more info:
https://github.com/encointer/pallets/tree/demo/xcm-personhood-oracle-and-sybil-gate-template#encointer-pallets
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

#### crates.io

use `cargo-release` we exclude non-public crates explicitly in tomls

```
cargo install cargo-release

# check workspace dependency tree
cargo tree --workspace -i ep-core

# add --execute if you're sure
cargo release publish -p ep-core -p encointer-primitives -p test-utils -p pallet-encointer-scheduler -p pallet-encointer-balances -p pallet-encointer-communities
cargo release publish -p encointer-ceremonies-assignment -p encointer-meetup-validation -p pallet-encointer-ceremonies -p pallet-encointer-bazaar -p pallet-encointer-reputation-commitments -p pallet-encointer-faucet
cargo release publish -p encointer-rpc -p encointer-balances-tx-payment -p encointer-balances-tx-payment-rpc-runtime-api -p encointer-balances-tx-payment-rpc -p pallet-encointer-bazaar-rpc-runtime-api -p pallet-encointer-bazaar-rpc -p pallet-encointer-ceremonies-rpc-runtime-api -p pallet-encointer-ceremonies-rpc -p pallet-encointer-communities-rpc-runtime-api -p pallet-encointer-communities-rpc
```

