use crate::{Pallet as Communities, *};
use encointer_primitives::{
	balances::Demurrage,
	communities::{CommunityMetadata, Location, NominalIncome},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{assert_ok, parameter_types};
use frame_system::RawOrigin;

const NUM_LOCATIONS: u32 = 500_000;

fn get_location(i: u32) -> Location {
	// splits the world into 1m locations
	assert!(i < 1_000_000);

	// lat from -40 to 40
	let lat = (i / 1000) as f64 * 0.08 - 40.0;

	// lon from -140 to 140
	let lon = (i % 1000) as f64 * 0.28 - 140.0;

	Location { lat: Degree::from_num(lat), lon: Degree::from_num(lon) }
}

parameter_types! {
	pub const DefaultDemurrage: Demurrage = Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);
}

//TODO extract test-community-setup in reused function or use the setup from test-utils

benchmarks! {
	new_community {
		let i in 2 .. NUM_LOCATIONS;

		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = Some("19charsaaaaaaaaa.ch".into());
		let demurrage = Some(Demurrage::from_num(DefaultDemurrage::get()));
		let nominal_income = Some(NominalIncome::from_num(1_u64));

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata.clone(), demurrage.clone(), nominal_income.clone()));
		let cid = CommunityIdentifier::new(get_location(0).clone(), bootstrappers.clone()).unwrap();
		for j in 1..i-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
	} : {
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(i-1), bootstrappers, community_metadata, demurrage, nominal_income));
	}
	verify { }

	add_location {
		let i in 2 .. NUM_LOCATIONS;

		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = None;

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata, None, None));
		let cid = CommunityIdentifier::new(get_location(0).clone(), bootstrappers).unwrap();
		for j in 1..i-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
	} : {
		assert_ok!(Communities::<T>::add_location(RawOrigin::Root.into(), cid, get_location(i-1)));
	}
	verify { }

	remove_location {
		let i in 2 .. NUM_LOCATIONS;

		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = None;

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata, None, None));
		let cid = CommunityIdentifier::new(get_location(0).clone(), bootstrappers).unwrap();
		for j in 1..i-1 {
			assert_ok!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)));
		}
	} : {
		assert_ok!(Communities::<T>::remove_location(RawOrigin::Root.into(), cid, get_location(i-1)));
	}
	verify { }

	update_community_metadata {
		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = Some("19charsaaaaaaaaa.ch".into());

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata.clone(), None, None));
		let cid = CommunityIdentifier::new(get_location(0), bootstrappers).unwrap();
	} : {
		assert_ok!(Communities::<T>::update_community_metadata(RawOrigin::Root.into(), cid, community_metadata));
	}
	verify { }

	update_demurrage {
		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = Some("19charsaaaaaaaaa.ch".into());
		let demurrage = Demurrage::from_num(DefaultDemurrage::get());

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata, None, None));
		let cid = CommunityIdentifier::new(get_location(0), bootstrappers).unwrap();
	} : {
		assert_ok!(Communities::<T>::update_demurrage(RawOrigin::Root.into(), cid, demurrage));
	}
	verify { }

	update_nominal_income {
		let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
		let mut community_metadata = CommunityMetadata::default();
		community_metadata.name = "20charsaaaaaaaaaaaaa".into();
		community_metadata.url = None;
		let nominal_income = NominalIncome::from_num(1_u64);

		// setup test community
		assert_ok!(Communities::<T>::new_community(RawOrigin::Root.into(), get_location(0), bootstrappers.clone(), community_metadata, None, None));
		let cid = CommunityIdentifier::new(get_location(0), bootstrappers).unwrap();
	} : {
		assert_ok!(Communities::<T>::update_nominal_income(RawOrigin::Root.into(), cid, nominal_income));
	}
	verify { }
}

// Don't runt the tests with cargo test, they take an hour
// impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
