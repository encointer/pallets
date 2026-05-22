use crate::*;
use encointer_primitives::balances::BalanceType;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

// Tolerance for balance comparisons under demurrage drift. 1e-4, same as the
// previous f64 `epsilon = 0.0001`. Computed in fixed-point so no float ops.
fn balance_tolerance() -> BalanceType {
	BalanceType::from_num(1u32) / 10_000
}

fn assert_balance_close(actual: BalanceType, expected: BalanceType) {
	let diff = if actual > expected { actual - expected } else { expected - actual };
	assert!(diff < balance_tolerance(), "balance {actual:?} not within tolerance of {expected:?}");
}

benchmarks! {
	transfer {
		let cid = CommunityIdentifier::default();
		let alice: T::AccountId = account("alice", 1, 1);
		let bob: T::AccountId = account("bob", 2, 2);

		Pallet::<T>::issue(cid, &alice, BalanceType::from_num(12i32)).ok();
	}: _(RawOrigin::Signed(alice.clone()), bob.clone(), cid, BalanceType::from_num(10i32))
	verify{
		assert_balance_close(Pallet::<T>::balance(cid, &alice), BalanceType::from_num(2u32));
		assert_balance_close(Pallet::<T>::balance(cid, &bob), BalanceType::from_num(10u32));
	}

	transfer_all {
		let cid = CommunityIdentifier::default();
		let alice: T::AccountId = account("alice", 1, 1);
		let bob: T::AccountId = account("bob", 2, 2);

		Pallet::<T>::issue(cid, &alice, BalanceType::from_num(12i32)).ok();
	}: _(RawOrigin::Signed(alice.clone()), bob.clone(), cid)
	verify{
		assert!(!Balance::<T>::contains_key(cid, alice));
		assert_balance_close(Pallet::<T>::balance(cid, &bob), BalanceType::from_num(12u32));
	}

	set_fee_conversion_factor {
		let alice: T::AccountId = account("alice", 1, 1);
		let f : FeeConversionFactorType = 1;
	}: _(RawOrigin::Root, f)
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
