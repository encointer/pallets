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

//! Mock runtime for the encointer_communities module

pub use crate as dut;
use frame_support::pallet_prelude::GenesisBuild;

use test_utils::*;

use encointer_primitives::scheduler::CeremonyPhaseType;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

frame_support::parameter_types! {
	pub const MinSolarTripTimeS: u32 = 1;
	pub const MaxSpeedMps: u32 = 83;
}

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerScheduler: encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerCommunities: dut::{Pallet, Call, Storage, Config<T>, Event<T>},
	}
);

impl dut::Config for TestRuntime {
	type Event = Event;
	type CouncilOrigin = EnsureAlice;
	type MinSolarTripTimeS = MinSolarTripTimeS;
	type MaxSpeedMps = MaxSpeedMps;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_scheduler!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
	dut::GenesisConfig::<TestRuntime> {
		community_master: Some(AccountKeyring::Alice.to_account_id()),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	encointer_scheduler::GenesisConfig::<TestRuntime> {
		current_phase: CeremonyPhaseType::REGISTERING,
		current_ceremony_index: 1,
		ceremony_master: Some(AccountKeyring::Alice.to_account_id()),
		phase_durations: vec![
			(CeremonyPhaseType::REGISTERING, ONE_DAY),
			(CeremonyPhaseType::ASSIGNING, ONE_DAY),
			(CeremonyPhaseType::ATTESTING, ONE_DAY),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}
