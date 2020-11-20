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


//! Unit tests for the tokens module.

use super::*;
use crate::{GenesisConfig, Module, Trait};
use encointer_currencies::{CurrencyIdentifier, Location, Degree};
use externalities::set_and_run_with_externalities;
use sp_core::{hashing::blake2_256, sr25519, Blake2Hasher, Pair, Public, H256};
use sp_runtime::traits::{CheckedAdd, IdentifyAccount, Member, Verify};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
use std::{cell::RefCell, collections::HashSet};
use frame_support::traits::{Currency, FindAuthor, Get, LockIdentifier};
use frame_support::{assert_ok, impl_outer_event, impl_outer_origin, parameter_types};
use sp_keyring::AccountKeyring;

use fixed::traits::LossyFrom;
use fixed::types::{I32F32, I9F23, I9F55};

const NONE: u64 = 0;
const REWARD: Balance = 1000;

/// The signature type used by accounts/transactions.
pub type Signature = sr25519::Signature;
/// An identifier for an account on this system.
pub type AccountId = <Signature as Verify>::Signer;


thread_local! {
    static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(2);
}
pub type BlockNumber = u64;
pub type Balance = u64;

pub struct ExistentialDeposit;
impl Get<u64> for ExistentialDeposit {
    fn get() -> u64 {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl encointer_balances::Trait for TestRuntime {
	type Event = ();
}

pub type EncointerBazaar = Module<TestRuntime>;

impl encointer_currencies::Trait for TestRuntime {
    type Event = ();
}
pub type EncointerCurrencies = encointer_currencies::Module<TestRuntime>;


parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: u32 = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for TestRuntime {
    type BaseCallFilter = ();       
    type Origin = Origin;
    type Index = u64;
    type Call = ();
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();    
    type MaximumBlockLength = MaximumBlockLength;
    type MaximumExtrinsicWeight = MaximumBlockWeight;    
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type AccountData = balances::AccountData<u64>;
	type OnNewAccount = ();
    type OnKilledAccount = ();  
    type SystemWeightInfo = (); 
    type PalletInfo = ();     
}

pub type System = frame_system::Module<TestRuntime>;

parameter_types! {
    pub const TransferFee: Balance = 0;
    pub const CreationFee: Balance = 0;
    pub const TransactionBaseFee: u64 = 0;
    pub const TransactionByteFee: u64 = 0;
}

impl balances::Trait for TestRuntime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();   
}
pub type Balances = balances::Module<TestRuntime>;

type AccountPublic = <Signature as Verify>::Signer;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

fn get_accountid(pair: &sr25519::Pair) -> AccountId {
    AccountPublic::from(pair.public()).into_account()
}

type T = Degree;

#[test]
fn new_shop_works() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        
        assert!(EncointerCurrencies::is_valid_geolocation(&a));
        assert!(EncointerCurrencies::is_valid_geolocation(&b));
        println!("testing Location {:?} and {:?}", a, b);
        println!("north pole at {:?}", NORTH_POLE);
        let loc = vec![a, b];
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        assert_ok!(EncointerCurrencies::new_currency(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        ));
        let cid = CurrencyIdentifier::from(blake2_256(&(loc.clone(), bs.clone()).encode()));
        let cids = EncointerCurrencies::currency_identifiers();
        assert!(cids.contains(&cid));
        assert_eq!(EncointerCurrencies::locations(&cid), loc);
        assert_eq!(EncointerCurrencies::bootstrappers(&cid), bs);
    });
}

#[test]
fn new_currency_with_too_close_inner_locations_fails() {
    ExtBuilder::build().execute_with(|| {
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let a = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1i32),
        };
        let b = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1.000001_f64),
        };
        // a and b roughly 11cm apart
        let loc = vec![a, b];
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        let cid = CurrencyIdentifier::default();

        assert!(EncointerCurrencies::new_currency(Origin::signed(alice.clone()), loc, bs).is_err());
    });
}