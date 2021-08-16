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
use crate::{Config, GenesisConfig, Module};
use frame_support::assert_ok;
use sp_core::{hashing::blake2_256, sr25519, Pair, H256};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use encointer_primitives::balances::consts::DEFAULT_DEMURRAGE;
use test_utils::{helpers::register_test_community, *};
extern crate alloc;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl Config for TestRuntime {
    type Event = ();
}

pub type System = frame_system::Pallet<TestRuntime>;
pub type EncointerCommunities = Module<TestRuntime>;

impl_frame_system!(TestRuntime);
impl_balances!(TestRuntime, System);
impl_encointer_communities!(TestRuntime);
impl_outer_origin_for_runtime!(TestRuntime);

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        encointer_balances::GenesisConfig {
            demurrage_per_block_default: Demurrage::from_bits(DEFAULT_DEMURRAGE),
        }
        .assimilate_storage(&mut storage)
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

fn string_to_geohash(s: &str) -> GeoHash {
    GeoHash(alloc::string::String::from(s).as_bytes().to_vec())
}

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
        let location = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1i32),
        };
        assert!(EncointerCommunities::is_valid_location(&location));
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        let community_meta: CommunityMetadataType = CommunityMetadataType {
            name: "Default".into(),
            symbol: "DEF".into(),
            ..Default::default()
        };
        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            community_meta.clone(),
            None,
            None
        ));
        let cid = CommunityIdentifier::from(blake2_256(&(location.clone(), bs.clone()).encode()));
        let cids = EncointerCommunities::community_identifiers();
        let geo_hash = GeoHash::try_from_params(location.lat, location.lon, BUCKET_RESOLUTION).unwrap();
        assert!(cids.contains(&cid));
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);
        assert_eq!(EncointerCommunities::bootstrappers(&cid), bs);
        assert_eq!(EncointerCommunities::bootstrappers(&cid), bs);
        assert_eq!(
            EncointerCommunities::community_metadata(&cid),
            community_meta
        );
    });
}

#[test]
fn two_communities_in_same_bucket_works() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        let community_meta: CommunityMetadataType = CommunityMetadataType {
            name: "Default".into(),
            symbol: "DEF".into(),
            ..Default::default()
        };

        let location = Location {
            lat: T::from_num(0i32),
            lon: T::from_num(0i32),
        };
        let geo_hash = GeoHash::try_from_params(location.lat, location.lon, BUCKET_RESOLUTION).unwrap();
        let location2 = Location {
            lat: T::from_num(0),
            lon: T::from_num(-0.015),
        };
        let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon, BUCKET_RESOLUTION).unwrap();
        assert_eq!(geo_hash, geo_hash2);

        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            community_meta.clone(),
            None,
            None
        ));

        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location2,
            bs.clone(),
            community_meta.clone(),
            None,
            None
        ));

        let cid = CommunityIdentifier::from(blake2_256(&(location.clone(), bs.clone()).encode()));
        let cid2 = CommunityIdentifier::from(blake2_256(&(location2.clone(), bs.clone()).encode()));
        let cids = EncointerCommunities::community_identifiers();

        assert!(cids.contains(&cid));
        assert!(cids.contains(&cid2));

        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
        assert_eq!(EncointerCommunities::locations(&cid2, &geo_hash2), vec![location2]);

        let mut cids_by_geohash = EncointerCommunities::cids_by_geohash(&geo_hash);
        let mut expected_cids_by_geohash = vec![cid, cid2];

        cids_by_geohash.sort();
        expected_cids_by_geohash.sort();

        assert_eq!(cids_by_geohash, expected_cids_by_geohash);

    });
}


#[test]
fn updating_nominal_income_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        assert!(NominalIncome::try_get(cid).is_err());
        assert_ok!(EncointerCommunities::update_nominal_income(
            Origin::root(),
            cid,
            BalanceType::from_num(1.1),
        ));
        assert_eq!(
            NominalIncome::try_get(&cid).unwrap(),
            BalanceType::from_num(1.1)
        );
    });
}

#[test]
fn updating_demurrage_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        assert!(DemurragePerBlock::try_get(cid).is_err());
        assert_ok!(EncointerCommunities::update_demurrage(
            Origin::root(),
            cid,
            Demurrage::from_num(0.0001),
        ));
        assert_eq!(
            DemurragePerBlock::try_get(&cid).unwrap(),
            BalanceType::from_num(0.0001)
        );
    });
}

#[test]
fn add_location_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let location = Location {
            lat: T::from_num(0i32),
            lon: T::from_num(0i32),
        };
        let geo_hash = GeoHash::try_from_params(location.lat, location.lon, BUCKET_RESOLUTION).unwrap();
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

        // add location in same bucket
        let location2 = Location {
            lat: T::from_num(0),
            lon: T::from_num(-0.015),
        };
        let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon, BUCKET_RESOLUTION).unwrap();
        assert_eq!(geo_hash, geo_hash2);

        EncointerCommunities::add_location(Origin::root(), cid, location2);
        let mut locations = EncointerCommunities::locations(&cid, &geo_hash);
        let mut expected_locations = vec![location, location2];
        locations.sort();
        expected_locations.sort();
        assert_eq!(locations, expected_locations);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

        // add location in different bucket
        let location3 = Location {
            lat: T::from_num(0),
            lon: T::from_num(0.015),
        };
        let geo_hash3 = GeoHash::try_from_params(location3.lat, location3.lon, BUCKET_RESOLUTION).unwrap();

        EncointerCommunities::add_location(Origin::root(), cid, location3);
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash3), vec![location3]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash3), vec![cid]);
    });
}

#[test]
fn remove_location_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let location = Location {
            lat: T::from_num(0i32),
            lon: T::from_num(0i32),
        };
        let geo_hash = GeoHash::try_from_params(location.lat, location.lon, BUCKET_RESOLUTION).unwrap();
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

        // add location in same bucket
        let location2 = Location {
            lat: T::from_num(0),
            lon: T::from_num(-0.015),
        };
        let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon, BUCKET_RESOLUTION).unwrap();
        assert_eq!(geo_hash, geo_hash2);

        EncointerCommunities::add_location(Origin::root(), cid, location2);
        let mut locations = EncointerCommunities::locations(&cid, &geo_hash);
        let mut expected_locations = vec![location, location2];
        locations.sort();
        expected_locations.sort();
        assert_eq!(locations, expected_locations);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

        // remove first location
        EncointerCommunities::remove_location(Origin::root(), cid, location);
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location2]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

        // remove second location
        EncointerCommunities::remove_location(Origin::root(), cid, location2);
        assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![]);
        assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![]);

    });
}

#[test]
fn new_community_too_close_to_existing_community_fails() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let location = Location {
            lat: T::from_num(1i32),
            lon: T::from_num(1i32),
        };
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
        assert_ok!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            Default::default(),
            None,
            None
        ));

        // second community
        let location = Location {
            lat: T::from_num(1.000001_f64),
            lon: T::from_num(1.000001_f64),
        };
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            Default::default(),
            None,
            None
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

        let location = Location {
            lat: T::from_num(89),
            lon: T::from_num(60),
        };
        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            Default::default(),
            None,
            None
        )
        .is_err());

        let a = Location {
            lat: T::from_num(-89),
            lon: T::from_num(60),
        };

        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs,
            Default::default(),
            None,
            None
        )
        .is_err());
    });
}

#[test]
fn new_community_near_dateline_fails() {
    ExtBuilder::build().execute_with(|| {
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let charlie = AccountId::from(AccountKeyring::Charlie);
        let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

        let location = Location {
            lat: T::from_num(10),
            lon: T::from_num(179),
        };

        assert!(EncointerCommunities::new_community(
            Origin::signed(alice.clone()),
            location,
            bs.clone(),
            Default::default(),
            None,
            None
        )
        .is_err());
    });
}

#[test]
fn get_relevant_neighbor_buckets_works() {
    /// for the following test we are looking at the following neighboring geo hashes
    /// sbh2m	sbh2q	sbh2r
    /// sbh2j	sbh2n	sbh2p
    /// kzurv	kzury	kzurz
    /// the hash in the center(sbh2n) has the following specs:
    /// center: lat: 0.02197265625 , lon: 40.01220703125
    /// lat min: 0
    /// lat max: 0.0439453125
    /// lon min: 39.990234375
    /// lon max: 40.0341796875
    ///
    ExtBuilder::build().execute_with(|| {
        // center location should not make it necessary to check any other buckets
        let bucket = string_to_geohash("sbh2n");
        let center = Location {
            lat: T::from_num(0.02197265625),
            lon: T::from_num(40.01220703125),
        };
        let result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &center).unwrap();
        assert_eq!(result.len(), 0);

        // location in the top right corner should make it necessary to check sbh2q, sbh2r and sbh2p
        let location = Location {
            lat: T::from_num(0.043945312),
            lon: T::from_num(40.034179687),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2q"), string_to_geohash("sbh2r"), string_to_geohash("sbh2p")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // location in the right should make it necessary to check sbh2p
        let location = Location {
            lat: T::from_num(0.02197265625),
            lon: T::from_num(40.034179687),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2p")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // location in the bottom right should make it necessary to check sbh2p, kzury, kzurz
        let location = Location {
            lat: T::from_num(0.0000000001),
            lon: T::from_num(40.034179687),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2p"), string_to_geohash("kzury"), string_to_geohash("kzurz")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // bottom
        let location = Location {
            lat: T::from_num(0.0000000001),
            lon: T::from_num(40.01220703125),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("kzury")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // bottom left
        let location = Location {
            lat: T::from_num(0.0000000001),
            lon: T::from_num(39.990234376),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("kzury"), string_to_geohash("kzurv"), string_to_geohash("sbh2j")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // left
        let location = Location {
            lat: T::from_num(0.02197265625),
            lon: T::from_num(39.990234376),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2j")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);


        // top left
        let location = Location {
            lat: T::from_num(0.043945312),
            lon: T::from_num(39.990234376),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2m"), string_to_geohash("sbh2q"), string_to_geohash("sbh2j")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

        // top
        let location = Location {
            lat: T::from_num(0.043945312),
            lon: T::from_num(40.01220703125),
        };
        let mut result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
        let mut expected_result = vec![string_to_geohash("sbh2q")];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);

    });
}


#[test]
fn get_nearby_locations_works() {
    /// for the following test we are looking at the following neighboring geo hashes
    /// sbh2m	sbh2q	sbh2r
    /// sbh2j	sbh2n	sbh2p
    /// kzurv	kzury	kzurz
    /// the hash in the center(sbh2n) has the following specs:
    /// center: lat: 0.02197265625 , lon: 40.01220703125
    /// lat min: 0
    /// lat max: 0.0439453125
    /// lon min: 39.990234375
    /// lon max: 40.0341796875
    ///
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let cid2 = register_test_community::<TestRuntime>(None, 1.0, 1.0);


        // location in top right corner of sbh2n
        let location = Location {
            lat: T::from_num(0.043945312),
            lon: T::from_num(40.034179687),
        };
        // locations in sbh2n
        let location2 = Location {
            lat: T::from_num(0.02197265625),
            lon: T::from_num(40.01220703125),
        };
        let location3 = Location {
            lat: T::from_num(0.0000000001),
            lon: T::from_num(39.990234376),
        };
        // location in sbh2r
        let location4 = Location {
            lat: T::from_num(0.066),
            lon: T::from_num(40.056),
        };
        // location in sbh2q
        let location5 = Location {
            lat: T::from_num(0.066),
            lon: T::from_num(40.012),
        };
        // location in sbh2p
        let location6 = Location {
            lat: T::from_num(0.022),
            lon: T::from_num(40.056),
        };
        // location in kzurz
        let location7 = Location {
            lat: T::from_num(-0.022),
            lon: T::from_num(40.056),
        };
        // location far away
        let location8 = Location {
            lat: T::from_num(45),
            lon: T::from_num(45),
        };

        // same bucket, same cid
        EncointerCommunities::add_location(Origin::root(), cid, location2);
        // same bucket different cid
        EncointerCommunities::add_location(Origin::root(), cid2, location3);
        //different bucket, same cid
        EncointerCommunities::add_location(Origin::root(), cid, location4);
        // different bucket, different cid
        EncointerCommunities::add_location(Origin::root(), cid2, location5);
        // different bucket, different cid
        EncointerCommunities::add_location(Origin::root(), cid2, location6);
        // location far away, same cid
        EncointerCommunities::add_location(Origin::root(), cid, location7);
        // location far away different cid
        EncointerCommunities::add_location(Origin::root(), cid2, location8);

        let mut result = EncointerCommunities::get_nearby_locations(&location).unwrap();
        let mut expected_result = vec![location2, location3, location4, location5, location6];
        result.sort();
        expected_result.sort();
        assert_eq!(result, expected_result);


    });
}

#[test]
fn validate_location_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);

        // close location
        let location = Location {
            lat: T::from_num(0.000000001),
            lon: T::from_num(0.000000001),
        };
        assert!(EncointerCommunities::validate_location(&location).is_err());

        // ok location
        let location = Location {
            lat: T::from_num(-0.043945312),
            lon: T::from_num(-0.043945312),
        };
        assert!(EncointerCommunities::validate_location(&location).is_ok());

        // locations too close to dateline
        let location = Location {
            lat: T::from_num(0),
            lon: T::from_num(179.9),
        };
        assert!(EncointerCommunities::validate_location(&location).is_err());
        let location = Location {
            lat: T::from_num(0),
            lon: T::from_num(-179.9),
        };
        assert!(EncointerCommunities::validate_location(&location).is_err());

    });
}