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
use encointer_primitives::balances::{BalanceType, Demurrage};
use frame_support::{parameter_types, PalletId};
use sp_runtime::BuildStorage;
use test_utils::*;
use crate::payout::NativePayout;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

frame_support::parameter_types! {
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
	/// 0.000005
	pub const ExistentialDeposit: BalanceType = BalanceType::from_bits(0x0000000000000000000053e2d6238da4_u128);
}

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerScheduler: pallet_encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerBalances: pallet_encointer_balances::{Pallet, Call, Storage, Event<T>, Config<T>},
		EncointerTreasuries: dut::{Pallet, Event<T>},
	}
);

parameter_types! {
	pub const TreasuriesPalletId: PalletId = PalletId(*b"trsrysId");
}
impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = pallet_balances::Pallet<TestRuntime>;
	type PalletId = TreasuriesPalletId;
	type AssetKind = ();
	type Paymaster = NativePayout<AccountId, Balances>;
	type WeightInfo = ();
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_encointer_scheduler!(TestRuntime);
impl_encointer_balances!(TestRuntime);
impl_balances!(TestRuntime, System);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();
	t.into()
}
