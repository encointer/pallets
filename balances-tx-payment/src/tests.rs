use crate::balance_to_community_balance;
use rstest::*;

/// 1 micro KSM with 12 decimals
const ONE_MICRO_KSM: u128 = 1_000_000;

/// 1 community currency token with 18 decimals
const ONE_CC: u128 = 1_000_000_000_000_000_000;

#[rstest(ksm_balance, ceremony_reward, conversion_factor, expected_community_balance,
case(5 * ONE_MICRO_KSM, 20 * ONE_CC, 100, ONE_CC / 100),
case(10 * ONE_MICRO_KSM, 20 * ONE_CC, 100, ONE_CC / 50),
case(5 * ONE_MICRO_KSM, 10 * ONE_CC, 100, ONE_CC / 200),
)]
fn balance_to_community_balance_works(
	ksm_balance: u128,
	ceremony_reward: u128,
	conversion_factor: u32,
	expected_community_balance: u128,
) {
	let balance = balance_to_community_balance(ksm_balance, ceremony_reward, conversion_factor);
	assert_eq!(balance, expected_community_balance);
}
