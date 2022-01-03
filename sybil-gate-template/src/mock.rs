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

//! Mock runtime for the encointer sybil-gate template module

use super::*;
pub use crate as dut;
use frame_support::dispatch::DispatchInfo;
use scale_info::TypeInfo;
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
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerSybilGate: dut::{Pallet, Call, Storage, Event<T>},
	}
);

frame_support::parameter_types! {
	pub const IssuePersonhoodUniquenessRatingWeight: u64 = 5_000_000;
}

impl dut::Config for TestRuntime {
	type Event = Event;
	type Call = EmptyCall;
	type XcmSender = ();
	type Currency = Balances;
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
	type IssuePersonhoodUniquenessRatingWeight = IssuePersonhoodUniquenessRatingWeight;
}

#[derive(Clone, PartialEq, Eq, Debug, Decode, Encode, TypeInfo)]
pub struct EmptyCall(());
impl GetDispatchInfo for EmptyCall {
	fn get_dispatch_info(&self) -> DispatchInfo {
		Default::default()
	}
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_balances!(TestRuntime, System);

// genesis values
pub fn new_test_ext() -> sp_io::TestExternalities {
	frame_system::GenesisConfig::default()
		.build_storage::<TestRuntime>()
		.unwrap()
		.into()
}
