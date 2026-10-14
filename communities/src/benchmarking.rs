use crate::{Pallet as Communities, *};
use encointer_primitives::{
	balances::Demurrage,
	communities::{Location, NominalIncome},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{assert_ok, parameter_types};
use frame_system::RawOrigin;

use encointer_primitives::common::FromStr;

const NUM_LOCATIONS: u32 = 200;

// as it is complicated to compute sqrt in no_std
const NUM_LOCATIONS_SQRT: u32 = 32;

fn get_location(i: u32) -> Location {
	// returns locations that are very close, so many of them will map to the same geohash bucket
	// this is close to the worstcase scenario for the location validation algorithm
	assert!(i < NUM_LOCATIONS);

	// top left corner coordinates of the bucket u0qjb, in micro-degrees (1e-6°)
	const LON_BASE_MICRO: i64 = 47_460_932; // 47.460932°
	const LAT_BASE_MICRO: i64 = 8_437_509; //  8.437509°
	const STEP_MICRO: i64 = 1_000; //         0.001°
	const ONE_DEGREE_MICRO: i64 = 1_000_000;

	let row = (i / NUM_LOCATIONS_SQRT) as i64;
	let col = (i % NUM_LOCATIONS_SQRT) as i64;

	let lat = Degree::from_num(LAT_BASE_MICRO + row * STEP_MICRO)
		.checked_div(Degree::from_num(ONE_DEGREE_MICRO))
		.expect("ONE_DEGREE_MICRO != 0; qed");
	let lon = Degree::from_num(LON_BASE_MICRO + col * STEP_MICRO)
		.checked_div(Degree::from_num(ONE_DEGREE_MICRO))
		.expect("ONE_DEGREE_MICRO != 0; qed");

	Location { lat, lon }
}

fn setup_test_community<T: Config>() -> (
	CommunityIdentifier,
	Vec<T::AccountId>,
	CommunityMetadataType,
	Option<Demurrage>,
	Option<NominalIncomeType>,
) {
	MaxSpeedMps::<T>::put(83);
	MinSolarTripTimeS::<T>::put(1);
	let bootstrappers: Vec<T::AccountId> = (0..10).map(|n| account("dummy name", n, n)).collect();

	let community_metadata = CommunityMetadataType {
		name: PalletString::from_str("20charsaaaaaaaaaaaaa").unwrap(),
		url: Some(PalletString::from_str("19charsaaaaaaaaa").unwrap()),
		..Default::default()
	};

	let demurrage = Some(Demurrage::from_num(DefaultDemurrage::get()));
	let nominal_income = Some(NominalIncome::from_num(1_u64));

	// setup test community
	assert_ok!(Communities::<T>::new_community(
		RawOrigin::Root.into(),
		get_location(0),
		bootstrappers.clone(),
		community_metadata.clone(),
		demurrage,
		nominal_income
	));
	let cid = CommunityIdentifier::new(get_location(0), bootstrappers.clone()).unwrap();

	(cid, bootstrappers, community_metadata, demurrage, nominal_income)
}

parameter_types! {
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
}

// 0.5 in I64F64 = 1 << 63
const HALF_DEMURRAGE: Demurrage = Demurrage::from_bits(1i128 << 63);

benchmarks! {
	new_community {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();

		for j in 1..NUM_LOCATIONS-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
		assert_eq!(Pallet::<T>::community_identifiers().len(), 1);
	} : {
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(NUM_LOCATIONS-1), bootstrappers, community_metadata, demurrage, nominal_income));
	}
	verify {
		assert_eq!(Pallet::<T>::community_identifiers().len(), 2);
	}

	add_location {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();

		for j in 1..NUM_LOCATIONS-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
		assert_eq!(Pallet::<T>::get_locations(&cid).len() as u32, NUM_LOCATIONS - 1);
	} : {
		assert_ok!(Communities::<T>::add_location(RawOrigin::Root.into(), cid, get_location(NUM_LOCATIONS-1)));
	}
	verify {
		assert_eq!(Pallet::<T>::get_locations(&cid).len() as u32, NUM_LOCATIONS);
	}

	remove_location {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();

		for j in 1..NUM_LOCATIONS-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
		assert_eq!(Pallet::<T>::get_locations(&cid).len() as u32, NUM_LOCATIONS - 1);
	} : {
		assert_ok!(Communities::<T>::remove_location(RawOrigin::Root.into(), cid, get_location(NUM_LOCATIONS-2)));
	}
	verify {
		assert_eq!(Pallet::<T>::get_locations(&cid).len() as u32, NUM_LOCATIONS - 2);
	}

	update_community_metadata {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();
		let mut new_community_metadata = CommunityMetadataType::default();
		let new_community_name: PalletString = PalletString::from_str("99charsaaaaaaaaaaaaa").unwrap();

		new_community_metadata.name = new_community_name.clone();
	} : {
		assert_ok!(Communities::<T>::update_community_metadata(RawOrigin::Root.into(), cid, new_community_metadata));
	}
	verify {
		assert_eq!(Pallet::<T>::community_metadata(cid).name, new_community_name);
	}

	update_demurrage {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();
	} : {
		assert_ok!(Communities::<T>::update_demurrage(RawOrigin::Root.into(), cid, HALF_DEMURRAGE));
	}
	verify {
		assert_eq!(pallet_encointer_balances::Pallet::<T>::demurrage_per_block(cid), HALF_DEMURRAGE);
	}

	update_nominal_income {
		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();
	} : {
		assert_ok!(Communities::<T>::update_nominal_income(RawOrigin::Root.into(), cid, NominalIncome::from(33u32)));
	}
	verify {
		assert_eq!(Pallet::<T>::nominal_income(cid), 33);
	}

	set_min_solar_trip_time_s {
	} : _(RawOrigin::Root, 1_000_000_000)
	verify {
		assert_eq!(Pallet::<T>::min_solar_trip_time_s(), 1_000_000_000);
	}

	set_max_speed_mps {
	} : _(RawOrigin::Root, 1_000_000_000)
	verify {
		assert_eq!(Pallet::<T>::max_speed_mps(), 1_000_000_000);
	}

	purge_community {
		// Todo: Properly benchmark this #189

		let (cid, bootstrappers, community_metadata, demurrage, nominal_income) = setup_test_community::<T>();
		let mut cids: BoundedVec<CommunityIdentifier, T::MaxCommunityIdentifiers> = BoundedVec::try_from(vec![CommunityIdentifier::default(); 9]).unwrap();
		cids.try_push(cid).ok();
		CommunityIdentifiers::<T>::put(cids);

	} : _(RawOrigin::Root, cid)
	verify {
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
