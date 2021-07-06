use crate::{*, Module as PalletModule};
use frame_benchmarking::{benchmarks, whitelisted_caller, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
   // new_community {
    //     let b in 1 .. 1_000_000;
    //     // The caller account is whitelisted for DB reads/write by the benchmarking macro.
    //     let caller: T::AccountId = whitelisted_caller();
    // }: _(RawOrigin::Signed(caller), b.into())
    sort_vector {
    let x in 0 .. 10000;
    let mut m = Vec::<u32>::new();
    for i in (0..x).rev() {
        m.push(i);
    }
	}: {
		// The benchmark execution phase could also be a closure with custom code
		m.sort();
	}
}


//impl_benchmark_test_suite!(PalletModule, crate::tests::new_test_ext(), crate::tests::Test);