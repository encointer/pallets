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
