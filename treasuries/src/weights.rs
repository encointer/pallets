#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

pub trait WeightInfo {
	fn swap_native() -> Weight;
	fn swap_asset() -> Weight;
}

// For tests
impl WeightInfo for () {
	fn swap_native() -> Weight {
		Weight::from_parts(5_100_000, 0)
			.saturating_add(RocksDbWeight::get().reads(10))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	fn swap_asset() -> Weight {
		Weight::from_parts(5_100_000, 0)
			.saturating_add(RocksDbWeight::get().reads(10))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
}
