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

//! Mock runtime for the encointer_balances module

use crate as dut;
use encointer_primitives::{balances::BalanceType, scheduler::CeremonyPhaseType};
use frame_support::{pallet_prelude::GenesisBuild, parameter_types};
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
		EncointerReputationCommitments: dut::{Pallet, Call, Storage, Event<T>},
		EncointerBalances: encointer_balances::{Pallet, Call, Storage, Event<T>},
		EncointerCommunities: encointer_communities::{Pallet, Call, Storage, Event<T>},
		EncointerCeremonies: encointer_ceremonies::{Pallet, Call, Storage, Config<T>, Event<T>},
	}
);

parameter_types! {}

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_scheduler!(TestRuntime, EncointerCeremonies, EncointerReputationCommitments);
impl_encointer_communities!(TestRuntime);
impl_encointer_balances!(TestRuntime);
impl_encointer_ceremonies!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

	let conf = dut::GenesisConfig {};
	GenesisBuild::<TestRuntime>::assimilate_storage(&conf, &mut t).unwrap();

	encointer_scheduler::GenesisConfig::<TestRuntime> {
		current_phase: CeremonyPhaseType::Registering,
		current_ceremony_index: 1,
		phase_durations: vec![
			(CeremonyPhaseType::Registering, ONE_DAY),
			(CeremonyPhaseType::Assigning, ONE_DAY),
			(CeremonyPhaseType::Attesting, ONE_DAY),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	encointer_ceremonies::GenesisConfig::<TestRuntime> {
		ceremony_reward: BalanceType::from_num(1),
		location_tolerance: LOCATION_TOLERANCE, // [m]
		time_tolerance: TIME_TOLERANCE,         // [ms]
		inactivity_timeout: 12,
		endorsement_tickets_per_bootstrapper: 50,
		endorsement_tickets_per_reputable: 2,
		reputation_lifetime: 3,
		meetup_time_offset: 0,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let conf = encointer_communities::GenesisConfig { min_solar_trip_time_s: 1, max_speed_mps: 83 };
	GenesisBuild::<TestRuntime>::assimilate_storage(&conf, &mut t).unwrap();

	t.into()
}
