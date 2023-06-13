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
use encointer_primitives::balances::BalanceType;
use frame_support::{pallet_prelude::GenesisBuild, parameter_types, traits::ConstU64, PalletId};
use sp_runtime::Permill;
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
		EncointerFaucet: dut::{Pallet, Call, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerBalances: encointer_balances::{Pallet, Call, Storage, Event<T>},
		EncointerCeremonies: encointer_ceremonies::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerReputationCommitments:encointer_reputation_commitments::{Pallet, Call, Storage, Event<T>},
		EncointerCommunities: encointer_communities::{Pallet, Call, Storage, Event<T>},
		Treasury: pallet_treasury::{Pallet, Call, Storage, Config, Event<T>},
	}
);

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const Burn: Permill = Permill::from_percent(50);
	pub const TreasuryPalletId: PalletId = PalletId(*b"py/trsry");
}
impl pallet_treasury::Config for TestRuntime {
	type PalletId = TreasuryPalletId;
	type Currency = pallet_balances::Pallet<TestRuntime>;
	type ApproveOrigin = EnsureAlice;
	type RejectOrigin = EnsureAlice;
	type RuntimeEvent = RuntimeEvent;
	type OnSlash = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ConstU64<1>;
	type ProposalBondMaximum = ();
	type SpendPeriod = ConstU64<2>;
	type Burn = Burn;
	type BurnDestination = (); // Just gets burned.
	type WeightInfo = ();
	type SpendFunds = ();
	type MaxApprovals = ConstU32<100>;
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<u64>;
}

impl pallet_balances::Config for TestRuntime {
	type MaxLocks = ();
	type MaxReserves = ConstU32<1000>;
	type ReserveIdentifier = [u8; 8];
	type Balance = u64;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU64<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type HoldIdentifier = ();
	type FreezeIdentifier = ();
	type MaxHolds = ();
	type MaxFreezes = ();
}

parameter_types! {
	pub const FaucetPalletId: PalletId = PalletId(*b"faucetId");
}

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type ControllerOrigin = EnsureAlice;
	type Currency = pallet_balances::Pallet<TestRuntime>;
	type PalletId = FaucetPalletId;
	type WeightInfo = ();
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_scheduler!(TestRuntime);
impl_encointer_balances!(TestRuntime);
impl_encointer_communities!(TestRuntime);
impl_encointer_ceremonies!(TestRuntime);
impl_encointer_reputation_commitments!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

	let conf = dut::GenesisConfig { reserve_amount: 13 };
	GenesisBuild::<TestRuntime>::assimilate_storage(&conf, &mut t).unwrap();

	encointer_ceremonies::GenesisConfig::<TestRuntime> {
		ceremony_reward: BalanceType::from_num(1),
		location_tolerance: LOCATION_TOLERANCE, // [m]
		time_tolerance: TIME_TOLERANCE,         // [ms]
		inactivity_timeout: 12,
		endorsement_tickets_per_bootstrapper: 50,
		endorsement_tickets_per_reputable: 2,
		reputation_lifetime: 6,
		meetup_time_offset: 0,
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let conf = encointer_communities::GenesisConfig { min_solar_trip_time_s: 1, max_speed_mps: 83 };
	GenesisBuild::<TestRuntime>::assimilate_storage(&conf, &mut t).unwrap();

	t.into()
}
