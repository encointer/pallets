use crate::{*, Module as PalletModule};
use frame_benchmarking::{benchmarks, whitelisted_caller, impl_benchmark_test_suite, account};
use frame_system::RawOrigin;
use encointer_primitives::{
    communities::{Location, CommunityMetadata, NominalIncome},
    balances:: {Demurrage, consts::DEFAULT_DEMURRAGE}
};

const NUM_LOCATIONS : u32 =  1000;
const NUM_BOOTSTRAPPERS : u32 = 12;

benchmarks! {
    new_community {
    let i in 1 .. NUM_LOCATIONS;
    let j in 3 .. NUM_BOOTSTRAPPERS;

    let caller: T::AccountId = whitelisted_caller();

    // spread locations along the equator, ie. lat = 0, lon equally spread over [-150, 150]
    // using 150 to have a save distance from the dateline
    let locations: Vec<Location> = (0..NUM_LOCATIONS)
    .map(|x| Location{lat: Degree::from_num(0.0), lon: Degree::from_num(((x as f64 * 300.0) / (NUM_LOCATIONS as f64)) - 150.0)})
    .collect();

    let bootrappers : Vec<T::AccountId> = (0..NUM_BOOTSTRAPPERS).map(|n| account("dummy name", n, n)).collect();

    let mut community_metadata = CommunityMetadata::default();
    community_metadata.name = "20charsaaaaaaaaaaaaa".into();
    community_metadata.url = Some("19charsaaaaaaaaa.ch".into());

    let demurrage = Some(Demurrage::from_num(DEFAULT_DEMURRAGE));
    let nominal_income = Some(NominalIncome::from_num(1));

	}: _(RawOrigin::Signed(caller), (&locations[..(i as usize)]).to_vec(), (&bootrappers[..(j as usize)]).to_vec(), community_metadata, demurrage, nominal_income)
}


//impl_benchmark_test_suite!(PalletModule, crate::tests::new_test_ext(), crate::tests::Test);