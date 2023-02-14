/*
Copyright 2022 Encointer

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.

*/

//! Autogenerated weights for pallet_encointer_balances with reference hardware:
//! * 2.3 GHz 8-Core Intel Core i9
//! * 16 GB 2400 MHz DDR4
//! * SSD
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-05-25, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// target/release/encointer-node-notee
// benchmark
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=pallet_encointer_balances
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=runtime/src/weights/pallet_encointer_balances.rs
// --template=/Users/pigu/Dropbox/DATA/Documents/solidbit/kunden/encointer/code/repos/encointer-node/scripts/frame-weight-template-full-info.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_encointer_balances.
pub trait WeightInfo {
	fn transfer() -> Weight;
	fn transfer_all() -> Weight;
	fn set_fee_conversion_factor() -> Weight;
}

/// Weights for pallet_encointer_balances using the Encointer solo chain node and recommended hardware.
pub struct EncointerWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for EncointerWeight<T> {
	fn transfer() -> Weight {
		Weight::from_ref_time(88_000_000)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	fn transfer_all() -> Weight {
		Weight::from_ref_time(111_000_000)
			.saturating_add(T::DbWeight::get().reads(5))
			.saturating_add(T::DbWeight::get().writes(4))
	}

	fn set_fee_conversion_factor() -> Weight {
		Weight::from_ref_time(16_300_000).saturating_add(T::DbWeight::get().writes(1))
	}
}

// For tests
impl WeightInfo for () {
	fn transfer() -> Weight {
		Weight::from_ref_time(88_000_000)
			.saturating_add(RocksDbWeight::get().reads(4))
			.saturating_add(RocksDbWeight::get().writes(3))
	}
	fn transfer_all() -> Weight {
		Weight::from_ref_time(111_000_000)
			.saturating_add(RocksDbWeight::get().reads(5))
			.saturating_add(RocksDbWeight::get().writes(4))
	}

	fn set_fee_conversion_factor() -> Weight {
		Weight::from_ref_time(16_300_000).saturating_add(RocksDbWeight::get().writes(1))
	}
}
