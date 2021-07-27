use crate::{*, Module as PalletModule};
use frame_benchmarking::{benchmarks, whitelisted_caller, impl_benchmark_test_suite, account};
use frame_system::RawOrigin;
use frame_system::Origin;
use encointer_primitives::{
    communities::{Location, CommunityMetadata, NominalIncome},
    balances:: {Demurrage, consts::DEFAULT_DEMURRAGE}
};

use log::{info, warn};

const NUM_LOCATIONS : u32 =  500000;

fn get_location(i:u32) -> Location {
    // splits the world into 1m locations
    let max_locations = 1000000;
    assert!(i < max_locations );

    // lat from -40 to 40
    let lat = (i / 1000) as f64 * 0.08 - 40.0;

    // lon from -140 to 140
    let lon = (i % 1000) as f64 * 0.28  - 140.0;

    Location{lat: Degree::from_num(lat), lon: Degree::from_num(lon)}
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
    let demurrage = Some(Demurrage::from_num(DEFAULT_DEMURRAGE));
    let nominal_income = Some(NominalIncome::from_num(1));

    // setup test community
    PalletModule::<T>::new_community(RawOrigin::Signed(caller.clone()).into(), get_location(0), bootstrappers.clone(), community_metadata.clone(), demurrage.clone(), nominal_income.clone());
    let cid = CommunityIdentifier::from(blake2_256(&(get_location(0).clone(), bootstrappers.clone()).encode()));
    for j in 1..i-1 {
        assert!(PalletModule::<T>::add_location(RawOrigin::Root.into(), cid, get_location(j)).is_ok());
    }
    warn!("setup complete.");
	}: _(RawOrigin::Signed(caller), get_location(i-1), bootstrappers, community_metadata, demurrage, nominal_income)
}


//impl_benchmark_test_suite!(PalletModule, crate::tests::new_test_ext(), crate::tests::Test);