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

use super::*;
use codec::Encode;
use encointer_primitives::communities::{CommunityIdentifier, Degree, Location};
use frame_support::assert_ok;
use frame_support::impl_outer_event;
use frame_system;
use sp_core::{hashing::blake2_256, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup};

use test_utils::*;

mod tokens {
    pub use crate::Event;
}
mod communities {
    pub use encointer_communities::Event;
}
impl_outer_event! {
    pub enum TestEvent for TestRuntime {
        tokens<T>,
        communities<T>,
        frame_system<T>,
    }
}

type AccountId = u64;

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl_outer_origin_for_runtime!(TestRuntime);
impl_frame_system!(TestRuntime, TestEvent);

pub type System = frame_system::Module<TestRuntime>;
pub type EncointerCommunities = encointer_communities::Module<TestRuntime>;
pub type EncointerBalances = Module<TestRuntime>;

impl encointer_communities::Config for TestRuntime {
    type Event = TestEvent;
}

impl Config for TestRuntime {
    type Event = TestEvent;
}

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub struct ExtBuilder {}

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

/// register a simple test community with 3 meetup locations and well known bootstrappers
pub fn register_test_community() -> CommunityIdentifier {
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
    assert_ok!(EncointerCommunities::new_community(
        Origin::signed(alice.clone()),
        loc.clone(),
        bs.clone()
    ));
    CommunityIdentifier::from(blake2_256(&(loc, bs).encode()))
}
