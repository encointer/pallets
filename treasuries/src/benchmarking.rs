use crate::*;
use encointer_primitives::treasuries::SwapNativeOption;
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::SaturatedConversion;

benchmarks! {
	where_clause {
		where
		H256: From<<T as frame_system::Config>::Hash>,
	}
	swap_native {
		let cid = CommunityIdentifier::default();
		let alice: T::AccountId = account("alice", 1, 1);
		let treasury = Pallet::<T>::get_community_treasury_account_unchecked(Some(cid));
		<T as Config>::Currency::make_free_balance_be(&treasury, 200_000_000u64.saturated_into());
		pallet_encointer_balances::Pallet::<T>::issue(cid, &alice, BalanceType::from_num(12i32)).unwrap();
		let swap_option: SwapNativeOption<BalanceOf<T>, T::Moment> = SwapNativeOption {
			cid,
			native_allowance: 100_000_000u64.saturated_into(),
			rate: Some(BalanceType::from_num(0.000_000_2)),
			do_burn: false,
			valid_from: None,
			valid_until: None,
		};
		Pallet::<T>::do_issue_swap_native_option(
			cid,
			&alice,
			swap_option
		).unwrap();
	} : _(RawOrigin::Signed(alice.clone()), cid, 50_000_000u64.saturated_into())
	verify {
		assert_eq!(<T as Config>::Currency::free_balance(&alice), 50_000_000u64.saturated_into());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
