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
use approx::assert_abs_diff_eq;
use frame_support::assert_ok;
use mock::{
	dut, master, new_test_ext, EncointerBalances, EncointerCommunities, Origin, System, TestRuntime,
};
use sp_core::sr25519;
use sp_runtime::DispatchError;

use encointer_primitives::balances::BalanceType;
use test_utils::{
	helpers::{account_id, assert_dispatch_err, bootstrappers, last_event},
	*,
};

type T = Degree;

fn string_to_geohash(s: &str) -> GeoHash {
	GeoHash::try_from(s).unwrap()
}

/// register a simple test community with a specified location and defined bootstrappers
pub fn register_test_community(
	custom_bootstrappers: Option<Vec<sr25519::Pair>>,
	lat: f64,
	lon: f64,
) -> CommunityIdentifier {
	let bs: Vec<AccountId> = custom_bootstrappers
		.unwrap_or_else(|| bootstrappers())
		.into_iter()
		.map(|b| account_id(&b))
		.collect();

	let prime = &bs[0];

	let location = Location { lat: Degree::from_num(lat), lon: Degree::from_num(lon) };
	dut::Pallet::<TestRuntime>::new_community(
		Origin::signed(prime.clone()),
		location.clone(),
		bs.clone(),
		Default::default(),
		None,
		None,
	)
	.unwrap();
	CommunityIdentifier::new(location.clone(), bs).unwrap()
}

#[test]
fn testdata_lat_long() {
	println!(" {} : {:x?} ", 1.1, Degree::from_num(1.1));
}

#[test]
fn solar_trip_time_works() {
	new_test_ext().execute_with(|| {
		// one degree equator
		let a = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let b = Location { lat: T::from_num(0i32), lon: T::from_num(1i32) }; // one degree lat is 111km at the equator
		assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 1099);
		assert_eq!(EncointerCommunities::solar_trip_time(&b, &a), 1099);
		// Reykjavik one degree lon: expect to yield much shorter times than at the equator
		let a = Location { lat: T::from_num(64.135480_f64), lon: T::from_num(-21.895410_f64) }; // this is reykjavik
		let b = Location { lat: T::from_num(64.135_480), lon: T::from_num(-20.895410) };
		assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 344);

		// Reykjavik 111km: expect to yield much shorter times than at the equator because
		// next time zone is much closer in meter overland.
		// -> require locations to be further apart (in east-west) at this latitude
		let a = Location { lat: T::from_num(64.135480_f64), lon: T::from_num(0_f64) }; // this is at reykjavik lat
		let b = Location { lat: T::from_num(64.135480_f64), lon: T::from_num(2.290000_f64) }; // 2.29° is 111km
		assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 789);
		// maximal
		let a = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let b = Location { lat: T::from_num(0i32), lon: T::from_num(180i32) };
		assert_eq!(EncointerCommunities::solar_trip_time(&a, &b), 110318);
		assert_eq!(EncointerCommunities::solar_trip_time(&b, &a), 110318);
	})
}

#[test]
fn haversine_distance_works() {
	new_test_ext().execute_with(|| {
		// compare in [km] for human readability

		// one degree lon at equator
		let a = Location { lat: T::from_num(0), lon: T::from_num(0) };
		let b = Location { lat: T::from_num(0), lon: T::from_num(1) };
		assert_abs_diff_eq!(
			f64::from(EncointerCommunities::haversine_distance(&a, &b) as i32) * 0.001,
			111111.0 * 0.001,
			epsilon = 0.1
		);

		// half equator
		let a = Location { lat: T::from_num(0), lon: T::from_num(0) };
		let b = Location { lat: T::from_num(0), lon: T::from_num(180) };
		assert_abs_diff_eq!(
			f64::from(EncointerCommunities::haversine_distance(&a, &b) as i32) * 0.001,
			12742.0,
			epsilon = 0.1
		);

		// pole to pole
		assert_abs_diff_eq!(
			f64::from(EncointerCommunities::haversine_distance(&NORTH_POLE, &SOUTH_POLE) as i32) *
				0.001,
			12742.0,
			epsilon = 0.1
		);
	});
}

#[test]
fn new_community_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);
		let location = Location { lat: T::from_num(1i32), lon: T::from_num(1i32) };
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

		let cid = CommunityIdentifier::new(location.clone(), bs.clone()).unwrap();
		assert_eq!(last_event::<TestRuntime>(), Some(Event::CommunityRegistered(cid).into()));

		let cids = EncointerCommunities::community_identifiers();
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();
		assert!(cids.contains(&cid));
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);
		assert_eq!(EncointerCommunities::bootstrappers(&cid), bs);
		assert_eq!(EncointerCommunities::bootstrappers(&cid), bs);
		assert_eq!(EncointerCommunities::community_metadata(&cid), community_meta);
	});
}

#[test]
fn new_community_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let bob = AccountId::from(AccountKeyring::Bob);
		assert_dispatch_err(
			EncointerCommunities::new_community(
				Origin::signed(bob),
				Location::default(),
				vec![],
				CommunityMetadataType::default(),
				None,
				None,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn two_communities_in_same_bucket_works() {
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);
		let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
		let bs2 = vec![bob.clone(), charlie.clone(), alice.clone()];
		let community_meta: CommunityMetadataType = CommunityMetadataType {
			name: "Default".into(),
			symbol: "DEF".into(),
			..Default::default()
		};

		let location = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();
		let location2 = Location { lat: T::from_num(0), lon: T::from_num(-0.015) };
		let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon).unwrap();
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
			bs2.clone(),
			community_meta.clone(),
			None,
			None
		));

		let cid = CommunityIdentifier::new(location.clone(), bs.clone()).unwrap();
		let cid2 = CommunityIdentifier::new(location2.clone(), bs2.clone()).unwrap();
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
fn updating_community_metadata_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community(None, 0.0, 0.0);
		let new_metadata = CommunityMetadataType { name: "New".into(), ..Default::default() };

		assert_ok!(EncointerCommunities::update_community_metadata(
			Origin::signed(AccountKeyring::Alice.into()),
			cid,
			new_metadata.clone(),
		));
		assert_eq!(last_event::<TestRuntime>(), Some(Event::MetadataUpdated(cid).into()));
		assert_eq!(CommunityMetadata::<TestRuntime>::try_get(&cid).unwrap(), new_metadata);
	});
}

#[test]
fn updating_community_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		let new_metadata = CommunityMetadataType { name: "New".into(), ..Default::default() };

		assert_dispatch_err(
			EncointerCommunities::update_community_metadata(
				Origin::signed(AccountKeyring::Bob.into()),
				cid,
				new_metadata.clone(),
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn updating_nominal_income_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community(None, 0.0, 0.0);
		assert!(NominalIncome::<TestRuntime>::try_get(cid).is_err());
		let income = BalanceType::from_num(1.1);
		assert_ok!(EncointerCommunities::update_nominal_income(
			Origin::signed(AccountKeyring::Alice.into()),
			cid,
			income,
		));
		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::NominalIncomeUpdated(cid, income).into())
		);
		assert_eq!(NominalIncome::<TestRuntime>::try_get(&cid).unwrap(), income);
	});
}

#[test]
fn updating_nominal_income_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		assert_dispatch_err(
			EncointerCommunities::update_nominal_income(
				Origin::signed(AccountKeyring::Bob.into()),
				cid,
				BalanceType::from_num(1.1),
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn updating_demurrage_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community(None, 0.0, 0.0);
		assert!(encointer_balances::DemurragePerBlock::<TestRuntime>::try_get(cid).is_err());
		let demurrage = Demurrage::from_num(0.0001);
		assert_ok!(EncointerCommunities::update_demurrage(
			Origin::signed(AccountKeyring::Alice.into()),
			cid,
			demurrage,
		));
		assert_eq!(
			last_event::<TestRuntime>(),
			Some(Event::DemurrageUpdated(cid, demurrage).into())
		);
		assert_eq!(
			encointer_balances::DemurragePerBlock::<TestRuntime>::try_get(&cid).unwrap(),
			demurrage
		);
	});
}

#[test]
fn updating_demurrage_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		assert_dispatch_err(
			EncointerCommunities::update_demurrage(
				Origin::signed(AccountKeyring::Bob.into()),
				cid,
				Demurrage::from_num(0.0001),
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn add_location_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community(None, 0.0, 0.0);
		let some_bootstrapper = AccountId::from(AccountKeyring::Alice);

		let location = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// add location in same bucket
		let location2 = Location { lat: T::from_num(0), lon: T::from_num(-0.015) };
		let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon).unwrap();
		assert_eq!(geo_hash, geo_hash2);

		assert_ok!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location2,
		));

		assert_eq!(last_event::<TestRuntime>(), Some(Event::LocationAdded(cid, location2).into()));
		let mut locations = EncointerCommunities::locations(&cid, &geo_hash);
		let mut expected_locations = vec![location, location2];
		locations.sort();
		expected_locations.sort();
		assert_eq!(locations, expected_locations);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// add location in different bucket
		let location3 = Location { lat: T::from_num(0), lon: T::from_num(0.015) };
		let geo_hash3 = GeoHash::try_from_params(location3.lat, location3.lon).unwrap();

		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location3,
		)
		.ok();
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash3), vec![location3]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash3), vec![cid]);
	});
}

#[test]
fn add_new_location_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		assert_dispatch_err(
			EncointerCommunities::add_location(
				Origin::signed(AccountKeyring::Bob.into()),
				cid,
				Location::default(),
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn remove_community_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		let cid2 = register_test_community(None, 1.0, 1.0);
		let some_bootstrapper = AccountId::from(AccountKeyring::Alice);

		let location = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// add location in same bucket
		let location2 = Location { lat: T::from_num(0), lon: T::from_num(-0.015) };
		let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon).unwrap();
		assert_eq!(geo_hash, geo_hash2);

		let location3 = Location { lat: T::from_num(10), lon: T::from_num(-0.015) };
		let geo_hash3 = GeoHash::try_from_params(location3.lat, location3.lon).unwrap();

		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location2,
		)
		.ok();

		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location3,
		)
		.ok();

		let mut locations = EncointerCommunities::locations(&cid, &geo_hash);
		let mut expected_locations = vec![location, location2];
		locations.sort();
		expected_locations.sort();
		assert_eq!(locations, expected_locations);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// remove first location
		EncointerCommunities::remove_community(cid);

		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![]);
		assert_eq!(EncointerCommunities::locations(&cid2, &geo_hash3), vec![location3]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash3), vec![cid2]);
	});
}

#[test]
fn remove_location_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1); // this is needed to assert events
		let cid = register_test_community(None, 0.0, 0.0);
		let some_bootstrapper = AccountId::from(AccountKeyring::Alice);

		let location = Location { lat: T::from_num(0i32), lon: T::from_num(0i32) };
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon).unwrap();
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// add location in same bucket
		let location2 = Location { lat: T::from_num(0), lon: T::from_num(-0.015) };
		let geo_hash2 = GeoHash::try_from_params(location2.lat, location2.lon).unwrap();
		assert_eq!(geo_hash, geo_hash2);

		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location2,
		)
		.ok();
		let mut locations = EncointerCommunities::locations(&cid, &geo_hash);
		let mut expected_locations = vec![location, location2];
		locations.sort();
		expected_locations.sort();
		assert_eq!(locations, expected_locations);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// remove first location
		assert_ok!(EncointerCommunities::remove_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location,
		));
		assert_eq!(last_event::<TestRuntime>(), Some(Event::LocationRemoved(cid, location).into()));
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![location2]);
		assert_eq!(EncointerCommunities::cids_by_geohash(&geo_hash), vec![cid]);

		// remove second location
		EncointerCommunities::remove_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location2,
		)
		.ok();
		assert_eq!(EncointerCommunities::locations(&cid, &geo_hash), vec![]);
		assert_eq!(
			EncointerCommunities::cids_by_geohash(&geo_hash),
			Vec::<CommunityIdentifier>::new()
		);
	});
}

#[test]
fn remove_location_errs_with_invalid_origin() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		assert_dispatch_err(
			EncointerCommunities::remove_location(
				Origin::signed(AccountKeyring::Bob.into()),
				cid,
				Location::default(),
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn new_community_too_close_to_existing_community_fails() {
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);
		let location = Location { lat: T::from_num(1i32), lon: T::from_num(1i32) };
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
		let location = Location { lat: T::from_num(1.000001_f64), lon: T::from_num(1.000001_f64) };
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
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);
		let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

		let location = Location { lat: T::from_num(89), lon: T::from_num(60) };
		assert!(EncointerCommunities::new_community(
			Origin::signed(alice.clone()),
			location,
			bs.clone(),
			Default::default(),
			None,
			None
		)
		.is_err());

		let a = Location { lat: T::from_num(-89), lon: T::from_num(60) };

		assert!(EncointerCommunities::new_community(
			Origin::signed(alice.clone()),
			a,
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
	new_test_ext().execute_with(|| {
		let alice = AccountId::from(AccountKeyring::Alice);
		let bob = AccountId::from(AccountKeyring::Bob);
		let charlie = AccountId::from(AccountKeyring::Charlie);
		let bs = vec![alice.clone(), bob.clone(), charlie.clone()];

		let location = Location { lat: T::from_num(10), lon: T::from_num(179) };

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
#[test]
fn get_relevant_neighbor_buckets_works() {
	new_test_ext().execute_with(|| {
		// center location should not make it necessary to check any other buckets
		let bucket = string_to_geohash("sbh2n");
		let center = Location { lat: T::from_num(0.02197265625), lon: T::from_num(40.01220703125) };
		let result = EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &center).unwrap();
		assert_eq!(result.len(), 0);

		// location in the top right corner should make it necessary to check sbh2q, sbh2r and sbh2p
		let location = Location { lat: T::from_num(0.043945312), lon: T::from_num(40.034179687) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![
			string_to_geohash("sbh2q"),
			string_to_geohash("sbh2r"),
			string_to_geohash("sbh2p"),
		];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// location in the right should make it necessary to check sbh2p
		let location = Location { lat: T::from_num(0.02197265625), lon: T::from_num(40.034179687) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![string_to_geohash("sbh2p")];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// location in the bottom right should make it necessary to check sbh2p, kzury, kzurz
		let location = Location { lat: T::from_num(0.0000000001), lon: T::from_num(40.034179687) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![
			string_to_geohash("sbh2p"),
			string_to_geohash("kzury"),
			string_to_geohash("kzurz"),
		];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// bottom
		let location =
			Location { lat: T::from_num(0.0000000001), lon: T::from_num(40.01220703125) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![string_to_geohash("kzury")];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// bottom left
		let location = Location { lat: T::from_num(0.0000000001), lon: T::from_num(39.990234376) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![
			string_to_geohash("kzury"),
			string_to_geohash("kzurv"),
			string_to_geohash("sbh2j"),
		];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// left
		let location = Location { lat: T::from_num(0.02197265625), lon: T::from_num(39.990234376) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![string_to_geohash("sbh2j")];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// top left
		let location = Location { lat: T::from_num(0.043945312), lon: T::from_num(39.990234376) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![
			string_to_geohash("sbh2m"),
			string_to_geohash("sbh2q"),
			string_to_geohash("sbh2j"),
		];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);

		// top
		let location = Location { lat: T::from_num(0.043945312), lon: T::from_num(40.01220703125) };
		let mut result =
			EncointerCommunities::get_relevant_neighbor_buckets(&bucket, &location).unwrap();
		let mut expected_result = vec![string_to_geohash("sbh2q")];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);
	});
}

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
///
#[test]
fn get_nearby_locations_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		let cid2 = register_test_community(None, 1.0, 1.0);

		// location in top right corner of sbh2n
		let location = Location { lat: T::from_num(0.043945312), lon: T::from_num(40.034179687) };
		// locations in sbh2n
		let location2 =
			Location { lat: T::from_num(0.02197265625), lon: T::from_num(40.01220703125) };
		let location3 = Location { lat: T::from_num(0.0000000001), lon: T::from_num(39.990234376) };
		// location in sbh2r
		let location4 = Location { lat: T::from_num(0.066), lon: T::from_num(40.056) };
		// location in sbh2q
		let location5 = Location { lat: T::from_num(0.066), lon: T::from_num(40.012) };
		// location in sbh2p
		let location6 = Location { lat: T::from_num(0.022), lon: T::from_num(40.056) };
		// location in kzurz
		let location7 = Location { lat: T::from_num(-0.022), lon: T::from_num(40.056) };
		// location far away
		let location8 = Location { lat: T::from_num(45), lon: T::from_num(45) };
		let some_bootstrapper = AccountId::from(AccountKeyring::Alice);

		// same bucket, same cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location2,
		)
		.ok();
		// same bucket different cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location3,
		)
		.ok();
		//different bucket, same cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location4,
		)
		.ok();
		// different bucket, different cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location5,
		)
		.ok();
		// different bucket, different cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location6,
		)
		.ok();
		// location far away, same cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location7,
		)
		.ok();
		// location far away different cid
		EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location8,
		)
		.ok();

		let mut result = EncointerCommunities::get_nearby_locations(&location).unwrap();
		let mut expected_result = vec![location2, location3, location4, location5, location6];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);
	});
}

#[test]
fn validate_location_works() {
	new_test_ext().execute_with(|| {
		register_test_community(None, 0.0, 0.0);

		// close location
		let location = Location { lat: T::from_num(0.000000001), lon: T::from_num(0.000000001) };
		assert!(EncointerCommunities::validate_location(&location).is_err());

		// ok location
		let location = Location { lat: T::from_num(-0.043945312), lon: T::from_num(-0.043945312) };
		assert!(EncointerCommunities::validate_location(&location).is_ok());

		// locations too close to dateline
		let location = Location { lat: T::from_num(0), lon: T::from_num(179.9) };
		assert!(EncointerCommunities::validate_location(&location).is_err());
		let location = Location { lat: T::from_num(0), lon: T::from_num(-179.9) };
		assert!(EncointerCommunities::validate_location(&location).is_err());
	});
}

#[test]
fn get_locations_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community(None, 0.0, 0.0);
		let cid2 = register_test_community(None, 1.0, 1.0);

		let location0 = Location { lat: T::from_num(0.0), lon: T::from_num(0.0) };

		let location1 = Location { lat: T::from_num(2.0), lon: T::from_num(2.0) };

		let location2 = Location { lat: T::from_num(3.0), lon: T::from_num(3.0) };

		let location3 = Location { lat: T::from_num(4.0), lon: T::from_num(4.0) };

		let location4 = Location { lat: T::from_num(5.0), lon: T::from_num(5.0) };

		let location5 = Location { lat: T::from_num(6.0), lon: T::from_num(6.0) };
		let some_bootstrapper = AccountId::from(AccountKeyring::Alice);

		assert!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location1
		)
		.is_ok());
		assert!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location2
		)
		.is_ok());
		assert!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location3
		)
		.is_ok());
		assert!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid2,
			location4
		)
		.is_ok());
		assert!(EncointerCommunities::add_location(
			Origin::signed(some_bootstrapper.clone()),
			cid,
			location5
		)
		.is_ok());

		let mut result = EncointerCommunities::get_locations(&cid);
		let mut expected_result = vec![location0, location1, location3, location5];
		result.sort();
		expected_result.sort();
		assert_eq!(result, expected_result);
	});
}

#[test]
fn set_min_solar_trip_time_s_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCommunities::set_min_solar_trip_time_s(
				Origin::signed(AccountKeyring::Bob.into()),
				1u32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_min_solar_trip_time_s_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCommunities::set_min_solar_trip_time_s(Origin::signed(master()), 2u32));

		assert_eq!(EncointerCommunities::min_solar_trip_time_s(), 2u32);
		assert_ok!(EncointerCommunities::set_min_solar_trip_time_s(Origin::signed(master()), 3u32));

		assert_eq!(EncointerCommunities::min_solar_trip_time_s(), 3u32);
	});
}

#[test]
fn set_max_speed_mps_errs_with_bad_origin() {
	new_test_ext().execute_with(|| {
		assert_dispatch_err(
			EncointerCommunities::set_max_speed_mps(
				Origin::signed(AccountKeyring::Bob.into()),
				1u32,
			),
			DispatchError::BadOrigin,
		);
	});
}

#[test]
fn set_max_speed_mps_works() {
	new_test_ext().execute_with(|| {
		assert_ok!(EncointerCommunities::set_max_speed_mps(Origin::signed(master()), 2u32));

		assert_eq!(EncointerCommunities::max_speed_mps(), 2u32);
		assert_ok!(EncointerCommunities::set_max_speed_mps(Origin::signed(master()), 3u32));

		assert_eq!(EncointerCommunities::max_speed_mps(), 3u32);
	});
}

#[test]
fn get_all_balances_works() {
	new_test_ext().execute_with(|| {
		let alice = AccountKeyring::Alice.to_account_id();
		let bob = AccountKeyring::Bob.to_account_id();

		let cid = register_test_community(None, 0.0, 0.0);
		let cid2 = register_test_community(None, 1.0, 1.0);
		let cid3 = register_test_community(None, 2.0, 2.0);

		assert_ok!(EncointerBalances::issue(cid, &alice, BalanceType::from_num(100)));
		assert_ok!(EncointerBalances::issue(cid, &bob, BalanceType::from_num(50)));

		assert_ok!(EncointerBalances::issue(cid2, &alice, BalanceType::from_num(20)));
		assert_ok!(EncointerBalances::issue(cid2, &bob, BalanceType::from_num(30)));

		assert_ok!(EncointerBalances::issue(cid3, &bob, BalanceType::from_num(10)));

		let balances_alice = EncointerCommunities::get_all_balances(&alice);
		assert_eq!(balances_alice.len(), 2);
		assert_eq!(balances_alice[0].0, cid);
		assert_eq!(balances_alice[0].1.principal, 100);
		assert_eq!(balances_alice[1].0, cid2);
		assert_eq!(balances_alice[1].1.principal, 20);

		let balances_bob = EncointerCommunities::get_all_balances(&bob);
		assert_eq!(balances_bob.len(), 3);
		assert_eq!(balances_bob[0].0, cid);
		assert_eq!(balances_bob[0].1.principal, 50);
		assert_eq!(balances_bob[1].0, cid2);
		assert_eq!(balances_bob[1].1.principal, 30);
		assert_eq!(balances_bob[2].0, cid3);
		assert_eq!(balances_bob[2].1.principal, 10);
	});
}
