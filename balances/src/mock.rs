// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.


//! Mocks for the tokens module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use frame_support::{assert_noop, assert_ok};
use frame_system;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use sp_core::{hashing::blake2_256, sr25519, Blake2Hasher, Pair, Public, H256};
use encointer_currencies::{CurrencyIdentifier, Location, Degree};
use super::*;

impl_outer_origin! {
	pub enum Origin for TestRuntime {}
}

mod tokens {
	pub use crate::Event;
}
mod currencies {
	pub use encointer_currencies::Event;
}
impl_outer_event! {
	pub enum TestEvent for TestRuntime {
		tokens<T>,
		currencies<T>,
		frame_system<T>,
	}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

type AccountId = u64;
impl frame_system::Trait for TestRuntime {
    type BaseCallFilter = ();    
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();    
    type MaximumBlockLength = MaximumBlockLength;
    type MaximumExtrinsicWeight = MaximumBlockWeight;
	type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
	type AccountData = ();
	type OnNewAccount = ();
    type OnKilledAccount = ();   
    type SystemWeightInfo = (); 
    type PalletInfo = ();
}
pub type System = frame_system::Module<TestRuntime>;

impl encointer_currencies::Trait for TestRuntime {
    type Event = TestEvent;
}

pub type EncointerCurrencies = encointer_currencies::Module<TestRuntime>;

impl Trait for TestRuntime {
	type Event = TestEvent;
}

pub type EncointerBalances = Module<TestRuntime>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub struct ExtBuilder {
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {}
	}
}

impl ExtBuilder {

	pub fn build(self) -> runtime_io::TestExternalities {
		let t = frame_system::GenesisConfig::default()
			.build_storage::<TestRuntime>()
			.unwrap();
		t.into()
	}
}

/// register a simple test currency with 3 meetup locations and well known bootstrappers
pub fn register_test_currency() -> CurrencyIdentifier {
    // all well-known keys are boottrappers for easy testen afterwards
    let alice = 1;
    let bob = 2;
    let charlie = 3;
    let dave = 4;
    let eve = 5;
    let ferdie = 6;
    
    let a = Location::default(); // 0, 0
    
    let b = Location {
        lat: Degree::from_num(1),
        lon: Degree::from_num(1),
    };
    let c = Location {
        lat: Degree::from_num(2),
        lon: Degree::from_num(2),
    };
    let loc = vec![a, b, c];
    let bs = vec![
        alice.clone(),
        bob.clone(),
        charlie.clone(),
        dave.clone(),
        eve.clone(),
        ferdie.clone(),
    ];
    assert_ok!(EncointerCurrencies::new_currency(
        Origin::signed(alice.clone()),
        loc.clone(),
        bs.clone()
    ));
    CurrencyIdentifier::from(blake2_256(&(loc, bs).encode()))
}
