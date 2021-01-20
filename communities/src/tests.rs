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

use super::*;
use crate::{GenesisConfig, Module, Trait};
use frame_support::traits::Get;
use frame_support::{assert_ok, impl_outer_origin, parameter_types};
use sp_core::{hashing::blake2_256, sr25519, Pair, H256};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::{IdentifyAccount, Verify};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
use std::cell::RefCell;

/// The signature type used by accounts/transactions.
pub type Signature = sr25519::Signature;
/// An identifier for an account on this system.
pub type AccountId = <Signature as Verify>::Signer;

thread_local! {
    static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(1);
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

impl Trait for TestRuntime {
    type Event = ();
}

pub type EncointerCurrencies = Module<TestRuntime>;

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

type AccountPublic = <Signature as Verify>::Signer;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        balances::GenesisConfig::<TestRuntime> { balances: vec![] }
            .assimilate_storage(&mut storage)
            .unwrap();
        GenesisConfig::<TestRuntime> {
            currency_master: get_accountid(&AccountKeyring::Alice.pair()),
        }
        .assimilate_storage(&mut storage)
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
fn testdata_lat_long() {
    println!(" {} : {:x?} ", 1.1, Degree::from_num(1.1));
}

#[test]
fn solar_trip_time_works() {
    // one degree equator
    let a = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(0i32),
    };
    let b = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(1i32),
    }; // one degree lat is 111km at the equator
    assert_eq!(EncointerCurrencies::solar_trip_time(&a, &b), 1099);
    assert_eq!(EncointerCurrencies::solar_trip_time(&b, &a), 1099);
    // Reykjavik one degree lon: expect to yield much shorter times than at the equator
    let a = Location {
        lat: T::from_num(64.135480_f64),
        lon: T::from_num(-21.895410_f64),
    }; // this is reykjavik
    let b = Location {
        lat: T::from_num(64.135_480),
        lon: T::from_num(-20.895410),
    };
    assert_eq!(EncointerCurrencies::solar_trip_time(&a, &b), 344);

    // Reykjavik 111km: expect to yield much shorter times than at the equator because
    // next time zone is much closer in meter overland.
    // -> require locations to be further apart (in east-west) at this latitude
    let a = Location {
        lat: T::from_num(64.135480_f64),
        lon: T::from_num(0_f64),
    }; // this is at reykjavik lat
    let b = Location {
        lat: T::from_num(64.135480_f64),
        lon: T::from_num(2.290000_f64),
    }; // 2.29° is 111km
    assert_eq!(EncointerCurrencies::solar_trip_time(&a, &b), 789);
    // maximal
    let a = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(0i32),
    };
    let b = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(180i32),
    };
    assert_eq!(EncointerCurrencies::solar_trip_time(&a, &b), 110318);
    assert_eq!(EncointerCurrencies::solar_trip_time(&b, &a), 110318);
}

#[test]
fn haversine_distance_works() {
    ExtBuilder::build().execute_with(|| {
        // compare in [km] for human readability

        // one degree lon at equator
        let a = Location {
            lat: T::from_num(0),
            lon: T::from_num(0),
        };
        let b = Location {
            lat: T::from_num(0),
            lon: T::from_num(1),
        };
        assert_abs_diff_eq!(
            f64::from(EncointerCurrencies::haversine_distance(&a, &b) as i32) * 0.001,
            111111.0 * 0.001,
            epsilon = 0.1
        );

        // half equator
        let a = Location {
            lat: T::from_num(0),
            lon: T::from_num(0),
        };
        let b = Location {
            lat: T::from_num(0),
            lon: T::from_num(180),
        };
        assert_abs_diff_eq!(
            f64::from(EncointerCurrencies::haversine_distance(&a, &b) as i32) * 0.001,
            12742.0,
            epsilon = 0.1
        );

        // pole to pole
        assert_abs_diff_eq!(
            f64::from(EncointerCurrencies::haversine_distance(&NORTH_POLE, &SOUTH_POLE) as i32)
                * 0.001,
            12742.0,
            epsilon = 0.1
        );
    });
}

#[test]
fn new_currency_works() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let a = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1i32),
        };
        let b = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(2i32),
        };
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

        assert!(EncointerCurrencies::new_currency(Origin::signed(alice.clone()), loc, bs).is_err());
    });
}

#[test]
fn new_currency_too_close_to_existing_currency_fails() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let a = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1i32),
        };
        let b = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(2i32),
        };
        let loc = vec![a, b];
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        assert_ok!(EncointerCurrencies::new_currency(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        ));

        // second currency
        let a = Location {
            lat: T::from_num(1.000001_f64),
            lon: T::from_num(1.000001_f64),
        };
        let b = Location {
            lat: T::from_num(1.000001_f64),
            lon: T::from_num(2.000001_f64),
        };
        let loc = vec![a, b];
        assert!(EncointerCurrencies::new_currency(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        )
        .is_err());
    });
}

#[test]
fn new_currency_with_near_pole_locations_fails() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

        let a = Location {
            lat: T::from_num(89),
            lon: T::from_num(60),
        };
        let b = Location {
            lat: T::from_num(89),
            lon: T::from_num(-60),
        };
        let loc = vec![a, b];
        assert!(
            EncointerCurrencies::new_currency(Origin::signed(alice.clone()), loc, bs.clone())
                .is_err()
        );

        let a = Location {
            lat: T::from_num(-89),
            lon: T::from_num(60),
        };
        let b = Location {
            lat: T::from_num(-89),
            lon: T::from_num(-60),
        };
        let loc = vec![a, b];
        assert!(EncointerCurrencies::new_currency(Origin::signed(alice.clone()), loc, bs).is_err());
    });
}

#[test]
fn new_currency_near_dateline_fails() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

        let a = Location {
            lat: T::from_num(10),
            lon: T::from_num(179),
        };
        let b = Location {
            lat: T::from_num(11),
            lon: T::from_num(179),
        };
        let loc = vec![a, b];
        assert!(
            EncointerCurrencies::new_currency(Origin::signed(alice.clone()), loc, bs.clone())
                .is_err()
        );
    });
}
