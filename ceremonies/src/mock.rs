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

use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

use encointer_primitives::{
	balances::{BalanceType, Demurrage},
	ceremonies::{ClaimOfAttendance, ProofOfAttendance},
	scheduler::CeremonyPhaseType,
};

pub type TestClaim = ClaimOfAttendance<Signature, AccountId, Moment>;
pub type TestProofOfAttendance = ProofOfAttendance<Signature, AccountId>;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerScheduler: encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerCeremonies: dut::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerCommunities: encointer_communities::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerBalances: encointer_balances::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const ReputationLifetime: u32 = 6;
	pub const AmountNewbieTickets: u8 = 50;
	pub const MinSolarTripTimeS: u32 = 1;
	pub const MaxSpeedMps: u32 = 83;
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
}

impl dut::Config for TestRuntime {
	type Event = Event;
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
	type RandomnessSource = frame_support_test::TestRandomness<TestRuntime>;
	type ReputationLifetime = ReputationLifetime;
	type AmountNewbieTickets = AmountNewbieTickets;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_communities!(TestRuntime);
impl_encointer_scheduler!(TestRuntime, EncointerCeremonies);
impl_encointer_balances!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
	encointer_communities::GenesisConfig::<TestRuntime> {
		community_master: AccountId::from(AccountKeyring::Alice),
	}
	.assimilate_storage(&mut t)
	.unwrap();
	encointer_scheduler::GenesisConfig::<TestRuntime> {
		current_phase: CeremonyPhaseType::REGISTERING,
		current_ceremony_index: 1,
		ceremony_master: AccountId::from(AccountKeyring::Alice),
		phase_durations: vec![
			(CeremonyPhaseType::REGISTERING, ONE_DAY),
			(CeremonyPhaseType::ASSIGNING, ONE_DAY),
			(CeremonyPhaseType::ATTESTING, ONE_DAY),
		],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	dut::GenesisConfig::<TestRuntime> {
		ceremony_reward: BalanceType::from_num(1),
		location_tolerance: LOCATION_TOLERANCE, // [m]
		time_tolerance: TIME_TOLERANCE,         // [ms]
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}
