use crate::{*, Module as PalletModule};
use frame_benchmarking::{benchmarks, whitelisted_caller, impl_benchmark_test_suite, account};
use frame_system::RawOrigin;
use frame_system::Origin;
use encointer_primitives::{
    communities::{Location, CommunityMetadata, NominalIncome},
    balances:: {Demurrage, consts::DEFAULT_DEMURRAGE}
};

const NUM_LOCATIONS : u32 =  20000;

benchmarks! {
    new_community {
    let i in 1 .. NUM_LOCATIONS;
    let caller: T::AccountId = whitelisted_caller();

    // spread locations along the equator, ie. lat = 0, lon equally spread over [-150, 150]
    // using 150 to have a save distance from the dateline
    let locations: Vec<Location> = (0..i + 1)
    .map(|x| Location{lat: Degree::from_num(0.0), lon: Degree::from_num(((x as f64 * 300.0) / (NUM_LOCATIONS as f64)) - 150.0)})
    .collect();

    let bootstrappers : Vec<T::AccountId> = (0..12).map(|n| account("dummy name", n, n)).collect();
    let mut community_metadata = CommunityMetadata::default();
    community_metadata.name = "20charsaaaaaaaaaaaaa".into();
    community_metadata.url = Some("19charsaaaaaaaaa.ch".into());
    let demurrage = Some(Demurrage::from_num(DEFAULT_DEMURRAGE));
    let nominal_income = Some(NominalIncome::from_num(1));

    // setup test community
    PalletModule::<T>::new_community(RawOrigin::Signed(caller.clone()).into(), locations[0usize], bootstrappers.clone(), community_metadata.clone(), demurrage.clone(), nominal_income.clone());
    let cid = CommunityIdentifier::from(blake2_256(&(locations[0usize].clone(), bootstrappers.clone()).encode()));
    for j in 1..i {
        PalletModule::<T>::add_location(RawOrigin::Root.into(), cid, locations[j as usize]);
    }



	}: _(RawOrigin::Signed(caller), locations[i as usize], bootstrappers, community_metadata, demurrage, nominal_income)
}


//impl_benchmark_test_suite!(PalletModule, crate::tests::new_test_ext(), crate::tests::Test);