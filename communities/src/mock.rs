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
use encointer_primitives::scheduler::CeremonyPhaseType;
use frame_support::traits::ConstU32;
use sp_runtime::BuildStorage;
use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerScheduler: pallet_encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerCommunities: dut::{Pallet, Call, Storage, Event<T>},
		EncointerBalances: pallet_encointer_balances::{Pallet, Call, Storage, Event<T>},
	}
);

pub fn master() -> AccountId {
	AccountId::from(AccountKeyring::Alice)
}

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type CommunityMaster = EnsureAlice;
	type TrustableForNonDestructiveAction = EnsureAlice;
	type WeightInfo = ();
	type MaxCommunityIdentifiers = ConstU32<10>;
	type MaxBootstrappers = ConstU32<10>;
	type MaxLocationsPerGeohash = ConstU32<200>;
	type MaxCommunityIdentifiersPerGeohash = ConstU32<10>;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_scheduler!(TestRuntime);
impl_encointer_balances!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();

	pallet_encointer_scheduler::GenesisConfig::<TestRuntime> {
		current_phase: CeremonyPhaseType::Registering,
		current_ceremony_index: 1,
		phase_durations: vec![
			(CeremonyPhaseType::Registering, ONE_DAY),
			(CeremonyPhaseType::Assigning, ONE_DAY),
			(CeremonyPhaseType::Attesting, ONE_DAY),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	dut::GenesisConfig::<TestRuntime> {
		min_solar_trip_time_s: 1,
		max_speed_mps: 83,
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}
