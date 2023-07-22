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

//! Mock runtime for the encointer_ceremonies module

pub use crate as dut;
use frame_support::parameter_types;
use sp_runtime::BuildStorage;
use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

use encointer_primitives::{
	balances::{BalanceType, Demurrage},
	ceremonies::ProofOfAttendance,
	scheduler::CeremonyPhaseType,
};

pub type TestProofOfAttendance = ProofOfAttendance<Signature, AccountId>;

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerScheduler: encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerCeremonies: dut::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerCommunities: encointer_communities::{Pallet, Call, Storage, Event<T>},
		EncointerBalances: encointer_balances::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
	pub const MeetupSizeTarget: u64 = 10;
	pub const MeetupMinSize: u64 = 3;
	pub const MeetupNewbieLimitDivider: u64 = 3;
}

pub fn master() -> AccountId {
	AccountId::from(AccountKeyring::Alice)
}

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type CeremonyMaster = EnsureAlice;
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
	type RandomnessSource = frame_support_test::TestRandomness<TestRuntime>;
	type MeetupSizeTarget = MeetupSizeTarget;
	type MeetupMinSize = MeetupMinSize;
	type MeetupNewbieLimitDivider = MeetupNewbieLimitDivider;
	type WeightInfo = ();
	type MaxAttestations = ConstU32<10>;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_communities!(TestRuntime);
impl_encointer_scheduler!(TestRuntime, EncointerCeremonies);
impl_encointer_balances!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();
	encointer_scheduler::GenesisConfig::<TestRuntime> {
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
		ceremony_reward: BalanceType::from_num(1),
		location_tolerance: LOCATION_TOLERANCE, // [m]
		time_tolerance: TIME_TOLERANCE,         // [ms]
		inactivity_timeout: 3,
		endorsement_tickets_per_bootstrapper: 50,
		endorsement_tickets_per_reputable: 2,
		reputation_lifetime: 6,
		meetup_time_offset: 0,
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	encointer_communities::GenesisConfig::<TestRuntime> {
		min_solar_trip_time_s: 1,
		max_speed_mps: 83,
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}
