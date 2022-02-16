use crate::*;
use encointer_primitives::{
	balances::Demurrage,
	communities::{CommunityMetadata, Location, NominalIncome},
};
use frame_benchmarking::{account, benchmarks, whitelisted_caller};
use frame_support::parameter_types;
use frame_system::RawOrigin;
use log::warn;

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

benchmarks! {
	new_community {
	let i in 2 .. NUM_LOCATIONS;
	warn!("starting benchmark {:?}", i);
	let caller: T::AccountId = whitelisted_caller();

	let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
	let mut community_metadata = CommunityMetadata::default();
	community_metadata.name = "20charsaaaaaaaaaaaaa".into();
	community_metadata.url = Some("19charsaaaaaaaaa.ch".into());
	let demurrage = Some(Demurrage::from_num(DefaultDemurrage::get()));
	let nominal_income = Some(NominalIncome::from_num(1));

	// setup test community
	Pallet::<T>::new_community(RawOrigin::Signed(caller.clone()).into(), get_location(0), bootstrappers.clone(), community_metadata.clone(), demurrage.clone(), nominal_income.clone());
	let cid = CommunityIdentifier::new(get_location(0).clone(), bootstrappers.clone()).unwrap();
	for j in 1..i-1 {
		assert!(Pallet::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)).is_ok());
	}
	warn!("setup complete.");
	}: _(RawOrigin::Signed(caller), get_location(i-1), bootstrappers, community_metadata, demurrage, nominal_income)
}

//impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
