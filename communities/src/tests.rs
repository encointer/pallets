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
use crate::{GenesisConfig, Module, Config};
use frame_support::assert_ok;
use sp_core::{hashing::blake2_256, sr25519, Pair, H256};
use sp_keyring::AccountKeyring;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use test_utils::*;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl Config for TestRuntime {
    type Event = ();
}

pub type System = frame_system::Module<TestRuntime>;
pub type EncointerCommunities = Module<TestRuntime>;

impl_frame_system!(TestRuntime);
impl_balances!(TestRuntime);
impl_outer_origin_for_runtime!(TestRuntime);

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
            community_master: get_accountid(&AccountKeyring::Alice.pair()),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

fn get_accountid(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public()).into()
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
    assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 1099);
    assert_eq!(EncointerCommunities::solar_trip_time(&b, &a), 1099);
    // Reykjavik one degree lon: expect to yield much shorter times than at the equator
    let a = Location {
        lat: T::from_num(64.135480_f64),
        lon: T::from_num(-21.895410_f64),
    }; // this is reykjavik
    let b = Location {
        lat: T::from_num(64.135_480),
        lon: T::from_num(-20.895410),
    };
    assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 344);

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
    assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 789);
    // maximal
    let a = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(0i32),
    };
    let b = Location {
        lat: T::from_num(0i32),
        lon: T::from_num(180i32),
    };
    assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 110318);
    assert_eq!(EncointerCommunities::solar_trip_time(&b, &a), 110318);
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
            f64::from(EncointerCommunities::haversine_distance(&a, &b) as i32) * 0.001,
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
            f64::from(EncointerCommunities::haversine_distance(&a, &b) as i32) * 0.001,
            12742.0,
            epsilon = 0.1
        );

        // pole to pole
        assert_abs_diff_eq!(
            f64::from(EncointerCommunities::haversine_distance(&NORTH_POLE, &SOUTH_POLE) as i32)
                * 0.001,
            12742.0,
            epsilon = 0.1
        );
    });
}

#[test]
fn new_community_works() {
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
        assert!(EncointerCommunities::is_valid_geolocation(&a));
        assert!(EncointerCommunities::is_valid_geolocation(&b));
        println!("testing Location {:?} and {:?}", a, b);
        println!("north pole at {:?}", NORTH_POLE);
        let loc = vec![a, b];
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        ));
        let cid = CommunityIdentifier::from(blake2_256(&(loc.clone(), bs.clone()).encode()));
        let cids = EncointerCommunities::community_identifiers();
        assert!(cids.contains(&cid));
        assert_eq!(EncointerCommunities::locations(&cid), loc);
        assert_eq!(EncointerCommunities::bootstrappers(&cid), bs);
    });
}

#[test]
fn new_community_with_too_close_inner_locations_fails() {
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

        assert!(
            EncointerCommunities::new_community(Origin::signed(alice.clone()), loc, bs).is_err()
        );
    });
}

#[test]
fn new_community_too_close_to_existing_community_fails() {
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
        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        ));

        // second community
        let a = Location {
            lat: T::from_num(1.000001_f64),
            lon: T::from_num(1.000001_f64),
        };
        let b = Location {
            lat: T::from_num(1.000001_f64),
            lon: T::from_num(2.000001_f64),
        };
        let loc = vec![a, b];
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc.clone(),
            bs.clone()
        )
        .is_err());
    });
}

#[test]
fn new_community_with_near_pole_locations_fails() {
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
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc,
            bs.clone()
        )
        .is_err());

        let a = Location {
            lat: T::from_num(-89),
            lon: T::from_num(60),
        };
        let b = Location {
            lat: T::from_num(-89),
            lon: T::from_num(-60),
        };
        let loc = vec![a, b];
        assert!(
            EncointerCommunities::new_community(Origin::signed(alice.clone()), loc, bs).is_err()
        );
    });
}

#[test]
fn new_community_near_dateline_fails() {
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
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc,
            bs.clone()
        )
        .is_err());
    });
}

#[test]
fn new_currency_with_very_close_location_works() {
    // This panicked before using I64F64 for degree due to an overflow in fixed::transcendental::sqrt
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

        // |a - b| ~ 300 m
        let a = Location {
            lat: T::from_num(47.2705520547),
            lon: T::from_num(8.6401677132),
        };
        let b = Location {
            lat: T::from_num(47.2696129372),
            lon: T::from_num(8.6439979076),
        };
        let loc = vec![a, b];
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            loc,
            bs.clone()
        )
        .is_ok());
    });
}
