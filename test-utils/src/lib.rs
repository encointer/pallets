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

use encointer_balances::BalanceType;
use frame_support::parameter_types;
use frame_support::traits::Get;
use sp_core::sr25519;
use sp_runtime::traits::Verify;
use sp_runtime::Perbill;
use std::cell::RefCell;

pub use balances;
pub use sp_core::H256;
pub use sp_runtime::traits::BlakeTwo256;

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
pub type Signature = sr25519::Signature;
/// An identifier for an account on this system.
pub type AccountId = <Signature as Verify>::Signer;

pub type BlockNumber = u64;
pub type Balance = u64;

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
        impl_frame_system($t, ())
    };
    ($t:ident, $event:ident) => {
        impl frame_system::Trait for $t {
            type BaseCallFilter = ();
            type Origin = Origin;
            type Call = ();
            type Index = u64;
            type BlockNumber = BlockNumber;
            type Hash = H256;
            type Hashing = BlakeTwo256;
            type AccountId = AccountId;
            type Lookup = IdentityLookup<Self::AccountId>;
            type Header = Header;
            type Event = $event;
            type BlockHashCount = BlockHashCount;
            type MaximumBlockWeight = MaximumBlockWeight;
            type DbWeight = ();
            type BlockExecutionWeight = ();
            type ExtrinsicBaseWeight = ();
            type MaximumExtrinsicWeight = MaximumBlockWeight;
            type MaximumBlockLength = MaximumBlockLength;
            type AvailableBlockRatio = AvailableBlockRatio;
            type Version = ();
            type PalletInfo = ();
            type AccountData = balances::AccountData<u64>;
            type OnNewAccount = ();
            type OnKilledAccount = ();
            type SystemWeightInfo = ();
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
        impl timestamp::Trait for $t {
            type Moment = Moment;
            type OnTimestampSet = $scheduler;
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
    ($t:ident) => {
        impl balances::Trait for $t {
            type Balance = Balance;
            type Event = ();
            type DustRemoval = ();
            type ExistentialDeposit = ExistentialDeposit;
            type AccountStore = System;
            type WeightInfo = ();
            type MaxLocks = ();
        }
    };
}

#[macro_export]
macro_rules! impl_encointer_balances {
    ($t:ident) => {
        impl encointer_balances::Trait for $t {
            type Event = ();
        }
    };
}

#[macro_export]
macro_rules! impl_encointer_communities {
    ($t:ident) => {
        impl encointer_communities::Trait for $t {
            type Event = ();
        }
    };
}

#[macro_export]
macro_rules! test_runtime {
    ($t:ident, $scheduler:ident) => {
        impl_frame_system!($t);
        impl_balances!($t);
        impl_timestamp!($t, $scheduler);
        impl_outer_origin_for_runtime!($t);
    };
}

#[macro_export]
macro_rules! impl_encointer_ceremonies {
    ($t:ident) => {
        impl encointer_ceremonies::Trait for $t {
            type Event = ();
            type Public = AccountId;
            type Signature = Signature;
        }
    };
}

parameter_types! {
    pub const MomentsPerDay: u64 = 86_400_000; // [ms/d]
}

#[macro_export]
macro_rules! impl_encointer_scheduler {
    ($t:ident, $module:ident) => {
        impl encointer_scheduler::Trait for $t {
            type Event = ();
            type OnCeremonyPhaseChange = $module<$t>; //OnCeremonyPhaseChange;
            type MomentsPerDay = MomentsPerDay;
        }
    };
}

#[macro_export]
macro_rules! impl_outer_origin_for_runtime {
    ($t:ident) => {
        frame_support::impl_outer_origin! {
            pub enum Origin for $t {}
        }
    };
}
