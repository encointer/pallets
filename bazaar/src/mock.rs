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

use crate as dut;

use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
        Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
        EncointerScheduler: encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
        EncointerCommunities: encointer_communities::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerBalances: encointer_balances::{Pallet, Call, Storage, Event<T>, Config},
        EncointerBazaar: dut::{Pallet, Call, Storage, Event<T>},
    }
);

impl dut::Config for TestRuntime {
    type Event = Event;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_balances!(TestRuntime);
impl_encointer_communities!(TestRuntime);
impl_encointer_scheduler!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
    frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap().into()
}
