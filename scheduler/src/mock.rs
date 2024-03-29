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

//! Mock runtime for the encointer_scheduler module

pub use crate as dut;
use encointer_primitives::scheduler::CeremonyPhaseType;
use sp_runtime::BuildStorage;
use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

pub fn master() -> AccountId {
	AccountId::from(AccountKeyring::Alice)
}

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerScheduler: dut::{Pallet, Call, Storage, Config<T>, Event},
	}
);

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type CeremonyMaster = EnsureAlice;
	type OnCeremonyPhaseChange = (); //OnCeremonyPhaseChange;
	type MomentsPerDay = MomentsPerDay;
	type WeightInfo = ();
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);

// genesis values
pub fn new_test_ext(phase_duration: u64) -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();
	dut::GenesisConfig::<TestRuntime> {
		current_phase: CeremonyPhaseType::Registering,
		current_ceremony_index: 1,
		phase_durations: vec![
			(CeremonyPhaseType::Registering, phase_duration),
			(CeremonyPhaseType::Assigning, phase_duration),
			(CeremonyPhaseType::Attesting, phase_duration),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}
