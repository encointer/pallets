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

//extern crate externalities;
//extern crate test_client;
//extern crate node_primitives;

use encointer_primitives::balances::{BalanceType, Demurrage};
use frame_support::{ord_parameter_types, parameter_types, traits::Get};
use frame_system::EnsureSignedBy;
use polkadot_parachain::primitives::Sibling;
use sp_core::crypto::AccountId32;
use sp_runtime::{generic, traits::IdentifyAccount, MultiSignature, Perbill};
use std::cell::RefCell;
use xcm::v1::NetworkId;
use xcm_builder::SiblingParachainConvertsVia;

// convenience reexport such that the tests do not need to put sp-keyring in the Cargo.toml.
pub use sp_keyring::AccountKeyring;

// reexports for macro resolution
pub use encointer_balances;
pub use encointer_ceremonies;
pub use encointer_communities;
pub use encointer_scheduler;
pub use frame_support_test;
pub use frame_system;
pub use pallet_balances;
pub use pallet_timestamp;
pub use sp_runtime;

pub use sp_core::H256;
pub use sp_runtime::traits::{BlakeTwo256, Verify};

pub mod helpers;
pub mod storage;

pub const NONE: u64 = 0;
pub const GENESIS_TIME: u64 = 1_585_058_843_000;
pub const ONE_DAY: u64 = 86_400_000;
pub const BLOCKTIME: u64 = 3_600_000; //1h per block
pub const TIME_TOLERANCE: u64 = 600000; // [ms]
pub const LOCATION_TOLERANCE: u32 = 1000; // [m]
pub const ZERO: BalanceType = BalanceType::from_bits(0x0);

thread_local! {
	static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(0);
}
/// The signature type used by accounts/transactions.
pub type Signature = MultiSignature;
/// An identifier for an account on this system.
pub type AccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Address = sp_runtime::MultiAddress<AccountId, ()>;

pub type BlockNumber = u64;
pub type Balance = u64;

pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

pub struct ExistentialDeposit;
impl Get<u64> for ExistentialDeposit {
	fn get() -> u64 {
		EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

#[macro_export]
macro_rules! impl_frame_system {
	($t:ident) => {
		use sp_runtime::traits::IdentityLookup;
		impl frame_system::Config for $t {
			type BaseCallFilter = frame_support::traits::Everything;
			type BlockWeights = ();
			type BlockLength = ();
			type AccountId = AccountId;
			type Call = Call;
			type Lookup = IdentityLookup<Self::AccountId>;
			type Index = u64;
			type BlockNumber = BlockNumber;
			type Hash = H256;
			type Hashing = BlakeTwo256;
			type Header = Header;
			type Event = Event;
			type Origin = Origin;
			type BlockHashCount = BlockHashCount;
			type DbWeight = ();
			type Version = ();
			type PalletInfo = PalletInfo;
			type OnNewAccount = ();
			type OnKilledAccount = ();
			type AccountData = pallet_balances::AccountData<u64>;
			type SystemWeightInfo = ();
			type SS58Prefix = ();
			type OnSetCode = ();
			type MaxConsumers = frame_support::traits::ConstU32<16>;
		}
	};
}

pub type Moment = u64;
parameter_types! {
	pub const MinimumPeriod: Moment = 1;
}

#[macro_export]
macro_rules! impl_timestamp {
	($t:ident, $scheduler:ident) => {
		impl pallet_timestamp::Config for $t {
			type Moment = Moment;
			type OnTimestampSet = $scheduler;
			type MinimumPeriod = MinimumPeriod;
			type WeightInfo = ();
		}
	};
	($t:ident) => {
		impl pallet_timestamp::Config for $t {
			type Moment = Moment;
			type OnTimestampSet = ();
			type MinimumPeriod = MinimumPeriod;
			type WeightInfo = ();
		}
	};
}

parameter_types! {
	pub const TransferFee: Balance = 0;
	pub const CreationFee: Balance = 0;
	pub const TransactionBaseFee: u64 = 0;
	pub const TransactionByteFee: u64 = 0;
}

#[macro_export]
macro_rules! impl_balances {
	($t:ident, $system:ident) => {
		impl pallet_balances::Config for $t {
			type Balance = Balance;
			type Event = Event;
			type DustRemoval = ();
			type ExistentialDeposit = ExistentialDeposit;
			type AccountStore = System;
			type WeightInfo = ();
			type MaxLocks = ();
			type MaxReserves = ();
			type ReserveIdentifier = [u8; 8];
		}
	};
}

parameter_types! {
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
}

#[macro_export]
macro_rules! impl_encointer_balances {
	($t:ident) => {
		impl encointer_balances::Config for $t {
			type Event = Event;
			type DefaultDemurrage = DefaultDemurrage;
		}
	};
}

parameter_types! {
	pub const MinSolarTripTimeS: u32 = 1;
	pub const MaxSpeedMps: u32 = 83;
}

#[macro_export]
macro_rules! impl_encointer_communities {
	($t:ident) => {
		impl encointer_communities::Config for $t {
			type Event = Event;
			type CouncilOrigin = EnsureAlice;
			type MinSolarTripTimeS = MinSolarTripTimeS;
			type MaxSpeedMps = MaxSpeedMps;
		}
	};
}

#[macro_export]
macro_rules! test_runtime {
	($t:ident, $system:ident, $scheduler:ident) => {
		impl_frame_system!($t);
		impl_balances!($t, $system);
		impl_timestamp!($t, $scheduler);
		impl_outer_origin_for_runtime!($t);
	};
}

parameter_types! {
	pub const ReputationLifetime: u32 = 1;
	pub const AmountNewbieTickets: u8 = 50;
	pub const InactivityTimeout: u32 = 12;
}

#[macro_export]
macro_rules! impl_encointer_ceremonies {
	($t:ident) => {
		impl encointer_ceremonies::Config for $t {
			type Event = Event;
			type Public = <Signature as Verify>::Signer;
			type Signature = Signature;
			type RandomnessSource = frame_support_test::TestRandomness<$t>;
			type ReputationLifetime = ReputationLifetime;
			type AmountNewbieTickets = AmountNewbieTickets;
			type InactivityTimeout = InactivityTimeout;
		}
	};
}

parameter_types! {
	pub const MomentsPerDay: u64 = 86_400_000; // [ms/d]
}

#[macro_export]
macro_rules! impl_encointer_scheduler {
	($t:ident, $ceremonies:ident) => {
		impl encointer_scheduler::Config for $t {
			type Event = Event;
			type OnCeremonyPhaseChange = $ceremonies; //OnCeremonyPhaseChange;
			type MomentsPerDay = MomentsPerDay;
		}
	};
	($t:ident) => {
		impl encointer_scheduler::Config for $t {
			type Event = Event;
			type OnCeremonyPhaseChange = (); //OnCeremonyPhaseChange;
			type MomentsPerDay = MomentsPerDay;
		}
	};
}

parameter_types! {
	pub const RococoNetwork: NetworkId = NetworkId::Polkadot;
}

pub type LocationConverter = SiblingParachainConvertsVia<Sibling, AccountId>;

ord_parameter_types! {
	pub const Alice: AccountId32 = AccountId32::new([212, 53, 147, 199, 21, 253, 211, 28, 97, 20, 26, 189, 4, 169, 159, 214, 130, 44, 133, 88, 133, 76, 205, 227, 154, 86, 132, 231, 165, 109, 162, 125]);
}

/// Test origin for the communities pallet's `EnsureOrigin` associated type.
pub type EnsureAlice = EnsureSignedBy<Alice, AccountId32>;
