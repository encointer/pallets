use crate::{apply_fee_conversion_factor, ONE_MICRO_KSM};
use rstest::*;

/// one unit of community currency is a fixpoint with 64 fractional bits
const ONE_CC: u128 = 1 << 64;

#[rstest(ksm_balance, ceremony_reward, conversion_factor, expected_community_balance,
case(5 * ONE_MICRO_KSM, 20 * ONE_CC, 100_000, ONE_CC / 100),
case(10 * ONE_MICRO_KSM, 20 * ONE_CC, 100_000, ONE_CC / 50),
case(5 * ONE_MICRO_KSM, 10 * ONE_CC, 100_000, ONE_CC / 200),
case(5_000 * ONE_MICRO_KSM, 20 * ONE_CC, 100_000, ONE_CC * 10),
case(5 * ONE_MICRO_KSM, 20_000_000 * ONE_CC, 100_000, ONE_CC * 10_000),
case(5 * ONE_MICRO_KSM, 20_000_000 * ONE_CC, 50_000, ONE_CC * 5_000),
)]
fn balance_to_community_balance_works(
	ksm_balance: u128,
	ceremony_reward: u128,
	conversion_factor: u128,
	expected_community_balance: u128,
) {
	let balance = apply_fee_conversion_factor(ksm_balance, ceremony_reward, conversion_factor);
	assert_eq!(balance, expected_community_balance);
}
