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
use frame_support::impl_outer_event;
use frame_system;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

use encointer_primitives::balances::{consts::DEFAULT_DEMURRAGE, Demurrage};
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

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl_outer_origin_for_runtime!(TestRuntime);
impl_frame_system!(TestRuntime, TestEvent);

pub type System = frame_system::Module<TestRuntime>;
pub type EncointerBalances = Module<TestRuntime>;

impl encointer_communities::Config for TestRuntime {
    type Event = TestEvent;
}

impl Config for TestRuntime {
    type Event = TestEvent;
}

pub struct ExtBuilder {}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl ExtBuilder {
    pub fn build(self) -> runtime_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        GenesisConfig {
            demurrage_per_block_default: Demurrage::from_bits(DEFAULT_DEMURRAGE),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        storage.into()
    }
}
