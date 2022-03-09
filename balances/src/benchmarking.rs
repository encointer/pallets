use crate::*;
use approx::assert_abs_diff_eq;
use encointer_primitives::{balances::BalanceType, fixed::traits::LossyInto};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
	transfer {
		let cid = CommunityIdentifier::default();
		let alice: T::AccountId = account("alice", 1, 1);
		let bob: T::AccountId = account("bob", 2, 2);
		let bob_lookup: <T::Lookup as StaticLookup>::Source = T::Lookup::unlookup(bob.clone());

		Pallet::<T>::issue(cid, &alice, BalanceType::from_num(12i32)).ok();
	}: _(RawOrigin::Signed(alice.clone()), bob_lookup, cid, BalanceType::from_num(10i32))
	verify{
		let balance_alice: f64 = Pallet::<T>::balance(cid, &alice).lossy_into();
		assert_abs_diff_eq!(balance_alice, 2f64, epsilon= 0.0001);
		let balance_bob: f64 = Pallet::<T>::balance(cid, &bob).lossy_into();
		assert_abs_diff_eq!(balance_bob, 10f64, epsilon= 0.0001);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
