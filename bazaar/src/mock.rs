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

#![cfg(test)]

use super::*;
use frame_system;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};
use crate as encointer_bazaar;
use frame_support::parameter_types;
use frame_support::RuntimeDebug;
use codec::{Encode, Decode};
use balances;


use test_utils::*;
use frame_support::traits::InstanceFilter;

mod tokens {
    pub use crate::Event;
}

mod communities {
    pub use encointer_communities::Event;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
    Any,
}

impl Default for ProxyType {
    fn default() -> Self {
        ProxyType::Any
    }
}

impl InstanceFilter<Call> for ProxyType {
    fn filter(&self, c: &Call) -> bool {
        match self {
            ProxyType::Any => true,
        }
    }
    fn is_superset(&self, o: &Self) -> bool {
        self == &ProxyType::Any || self == o
    }
}

frame_support::construct_runtime!(
    pub enum TestRuntime where
        Block = Block,
        NodeBlock = Block,
        UncheckedExtrinsic = UncheckedExtrinsic,
    {
        System: frame_system::{ Call, Config, Storage, Event<T>},
        EncointerBazaar: encointer_bazaar::{ Call, Storage, Event<T>},
        EncointerCommunities: encointer_communities::{Event<T>},
        PalletProxy: pallet_proxy::{Call, Event<T>},
        Balances: balances::{Event<T>}
    }
);

type BalanceResolution = u64;

parameter_types! {
    pub const ExistentialDeposit: u64 = 1;
    pub const ProxyDepositBase: BalanceResolution = 1;
    pub const ProxyDepositFactor: BalanceResolution = 1;
    pub const MaxProxies: u16 = 1;
    pub const MaxPending: u16 = 1;
    pub const AnnouncementDepositBase: BalanceResolution = 1;
    pub const AnnouncementDepositFactor: BalanceResolution = 1;
}

impl frame_system::Config for TestRuntime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Call = Call;
    type Index = u64;
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = Event;
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = Info;
    type AccountData = balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type OnSetCode = ();
    type SystemWeightInfo = ();
    type BlockWeights = ();
    type BlockLength = ();
    type SS58Prefix = ();
}

impl pallet_proxy::Config for TestRuntime
{
    type Event = Event;
    type Call = Call;
    type Currency = Balances;
    type ProxyType = ProxyType;
    type ProxyDepositBase = ProxyDepositBase;
    type ProxyDepositFactor = ProxyDepositFactor;
    type MaxProxies = MaxProxies;
    type WeightInfo = pallet_proxy::weights::SubstrateWeight<TestRuntime>;
    type MaxPending = MaxPending;
    type CallHasher = BlakeTwo256;
    type AnnouncementDepositBase = AnnouncementDepositBase;
    type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl encointer_bazaar::Config for TestRuntime {
    type Event = Event;
}

impl encointer_communities::Config for TestRuntime {
    type Event = Event;
}

pub struct ExtBuilder {}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl balances::Config for TestRuntime {
    type Balance = BalanceResolution;
    type DustRemoval = ();
    type Event = Event;
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        storage.into()
    }
}