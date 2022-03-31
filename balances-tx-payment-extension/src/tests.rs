use crate::{
	balance_to_community_balance,
	mock::{new_test_ext, TestRuntime},
};
use approx::{assert_abs_diff_eq, assert_relative_eq};
use encointer_primitives::{
	balances::BalanceType,
	communities::CommunityIdentifier,
	fixed::{traits::LossyInto, transcendental::exp},
};
use frame_support::{
	assert_noop, assert_ok,
	traits::{
		tokens::{
			fungibles::{Inspect, InspectMetadata, Unbalanced},
			DepositConsequence, WithdrawConsequence,
		},
		OnInitialize,
	},
};
use test_utils::{
	helpers::{assert_dispatch_err, last_event, register_test_community},
	AccountKeyring,
};

#[test]
fn balance_to_community_balance_works() {
	new_test_ext().execute_with(|| {
		let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
		let ksm_balance: u128 = 5_233_000;
		let reward: u128 = 20u128 * 1e18 as u128;

		let balance =
			balance_to_community_balance::<TestRuntime>(ksm_balance, cid, reward, 10_000, 18)
				as f64 * 1e-18;
		assert_abs_diff_eq!(balance, 0.01, epsilon = 10e-9);

		let ksm_balance: u128 = 10_466_000;
		let reward: u128 = 20u128 * 1e18 as u128;

		let balance =
			balance_to_community_balance::<TestRuntime>(ksm_balance, cid, reward, 10_000, 18)
				as f64 * 1e-18;
		assert_abs_diff_eq!(balance, 0.02, epsilon = 10e-9);

		let ksm_balance: u128 = 10_466_000;
		let reward: u128 = 20u128 * 1e18 as u128;

		let balance = balance_to_community_balance::<TestRuntime>(
			ksm_balance,
			cid,
			reward,
			5_000,
			18,
		) as f64 * 1e-18;
		assert_abs_diff_eq!(balance, 0.01, epsilon = 10e-9);

		let ksm_balance: u128 = 10_466_000;
		let reward: u128 = 10u128 * 1e18 as u128;

		let balance = balance_to_community_balance::<TestRuntime>(
			ksm_balance,
			cid,
			reward,
			5_000,
			18,
		) as f64 * 1e-18;
		assert_abs_diff_eq!(balance, 0.005, epsilon = 10e-9);
	});
}
