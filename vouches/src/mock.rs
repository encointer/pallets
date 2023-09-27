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
use frame_support::{parameter_types, traits::ConstU32};
use sp_runtime::BuildStorage;
use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		EncointerVouches: dut::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {}

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxVouchesPerAttester = ConstU32<4>;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	let t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();
	t.into()
}
