// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

use codec::{Decode, Encode, MaxEncodedLen};
use ep_core::fixed::types::I64F64;
use log::{trace, warn};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;
use sp_std::fmt::Debug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::{AtLeast32Bit, Convert};

use crate::fixed::{
	traits::ToFixed,
	transcendental::exp,
	types::{U64F64, U66F62},
};
#[cfg(feature = "serde_derive")]
use ep_core::serde::serialize_fixed;

const LOG: &str = "encointer::demurrage";

// We're working with fixpoint here.

/// Encointer balances are fixpoint values
pub type BalanceType = U64F64;

/// Demurrage is the rate of evanescence of balances per block
/// it must be positive
/// 0.0 : no demurrage at all
/// 1.3188e-07 : halving time of 1 year if blocktime is 6s
pub type Demurrage = I64F64;

pub type FeeConversionFactorType = u128;

#[derive(
	Encode, Decode, Default, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct BalanceEntry<BlockNumber> {
	/// The balance of the account after last manual adjustment
	#[cfg_attr(feature = "serde_derive", serde(with = "serialize_fixed"))]
	pub principal: BalanceType,
	/// The time (block height) at which the balance was last adjusted
	pub last_update: BlockNumber,
}

impl<BlockNumber> BalanceEntry<BlockNumber>
where
	BlockNumber: AtLeast32Bit + Debug,
{
	pub fn new(principal: BalanceType, last_update: BlockNumber) -> Self {
		Self { principal, last_update }
	}

	/// Applies the demurrage and returns an updated BalanceEntry.
	///
	/// The following formula is applied to the principal:
	///    updated_principal = old_principal * e^(-demurrage_per_block * elapsed_blocks)
	///
	/// If the `demurrage_per_block` is negative it will take the absolute value of
	/// it for the calculation.
	///
	/// **Note**: This function will be used at every single transaction that is paid with community
	/// currency. It is important that it is as efficient as possible, but also that it is bullet-
	/// proof (no-hidden panics!!).
	pub fn apply_demurrage(
		self,
		demurrage_per_block: Demurrage,
		current_block_number: BlockNumber,
	) -> BalanceEntry<BlockNumber> {
		if self.last_update == current_block_number {
			// Nothing to be done, as no time elapsed.
			return self
		}

		if self.principal.eq(&0i16) {
			return Self { principal: self.principal, last_update: current_block_number }
		}

		let elapsed_blocks =
			current_block_number.checked_sub(&self.last_update).unwrap_or_else(|| {
				// This should never be the case on a sound blockchain setup, but we really
				// never want to panic here.
				warn!(
					target: LOG,
					"last block {:?} bigger than current {:?}, defaulting to elapsed_blocks=0",
					self.last_update,
					current_block_number
				);
				0u32.into()
			});

		let elapsed_u32: u32 = elapsed_blocks.try_into().unwrap_or_else(|_| {
			// 698 years with 6 seconds block time.
			trace!(target: LOG, "elapsed_blocks > u32::MAX, defaulting to u32::MAX");
			u32::MAX
		});

		let demurrage_factor = demurrage_factor(demurrage_per_block, elapsed_u32);

		let principal = self
			.principal
			.checked_mul(demurrage_factor)
			.expect("demurrage_factor [0,1), hence can't overflow; qed");

		Self { principal, last_update: current_block_number }
	}
}

/// e^(-demurrage_per_block * elapsed_blocks) within [0,1).
///
/// It will take the absolute value of the `demurrage_per_block` if it is negative.
pub fn demurrage_factor(demurrage_per_block: Demurrage, elapsed_blocks: u32) -> BalanceType {
	// We can only have errors here if one of the operations overflowed.
	//
	// However, as we compute exp(-x), which goes to 0 for big x, we can
	// approximate the result as 0 if we overflow somewhere on the way
	// because of the big x.

	// demurrage >= 0; hence exponent <= 0
	let exponent = match (-demurrage_per_block.abs()).checked_mul(elapsed_blocks.into()) {
		Some(exp) => exp,
		None => return 0u64.into(),
	};

	// exponent <= 0; hence return value [0, 1)
	let f: I64F64 = exp(exponent).unwrap_or_else(|_| 0.into());

	// Safe conversion. The result of an exponential function can't be negative.
	BalanceType::from_le_bytes(f.to_le_bytes())
}

/// Our BalanceType is I64F64, so the smallest possible number is
/// 2^-64 = 5.42101086242752217003726400434970855712890625 × 10^-20
/// and the upper bound is 2^63 + 1 = 9.223372036854775809 × 10^18
///
/// We choose 18 decimals and lose some precision, but can prevent overflows that way.
pub const ENCOINTER_BALANCE_DECIMALS: u32 = 18;

pub const ONE_ENCOINTER_BALANCE_UNIT: u128 = 1_000_000_000_000_000_000;

pub struct EncointerBalanceConverter;

// Todo: Make u128 generic
impl Convert<BalanceType, u128> for EncointerBalanceConverter {
	fn convert(balance: BalanceType) -> u128 {
		let bits = balance.to_bits();
		let mut result: u128 = 0;

		result += (bits >> 64) as u128 * ONE_ENCOINTER_BALANCE_UNIT;

		result += BalanceType::from_bits((bits as u64) as u128) // <- to truncate
			.saturating_mul_int(ONE_ENCOINTER_BALANCE_UNIT)
			.to_num::<u128>();
		result
	}
}

impl Convert<u128, BalanceType> for EncointerBalanceConverter {
	fn convert(fungible_balance: u128) -> BalanceType {
		let mut result: BalanceType = BalanceType::from_num(0u128);

		// compute fractional part
		let f64_part = U64F64::from_num(fungible_balance as u64) // <- truncate integer bits
			.checked_div_int(ONE_ENCOINTER_BALANCE_UNIT)
			.expect("Divisor is > 1, no overflow or division by 0 can occur; qed")
			.to_fixed::<BalanceType>();

		result += f64_part;

		// compute integer part
		let conversion_factor = U66F62::from_num(2u128.pow(64))
			.checked_div_int(ONE_ENCOINTER_BALANCE_UNIT)
			.expect("Divisor is > 1, no overflow or division by 0 can occur; qed");

		let i64_part = BalanceType::from_num(fungible_balance >> 64)
			.saturating_mul(conversion_factor.to_fixed());

		result += i64_part;
		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fixed::traits::{LossyFrom, LossyInto};
	use approx::assert_abs_diff_eq;
	use rstest::*;
	use test_utils::helpers::almost_eq;

	const ONE_YEAR: u32 = 86400 / 5 * 356;

	// 1.1267607882072287e-7
	const DEFAULT_DEMURRAGE: Demurrage =
		Demurrage::from_bits(0x0000000000000000000001E3F0A8A973_i128);

	fn assert_abs_diff_eq(balance: BalanceType, expected: f64) {
		assert_abs_diff_eq!(f64::lossy_from(balance), expected, epsilon = 1.0e-12)
	}

	#[test]
	fn demurrage_works() {
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);
		assert_abs_diff_eq(bal.apply_demurrage(DEFAULT_DEMURRAGE, ONE_YEAR).principal, 0.5);
	}

	#[test]
	fn apply_demurrage_when_principal_is_zero_works() {
		let bal = BalanceEntry::<u32>::new(0u32.into(), 0);
		assert_abs_diff_eq(bal.apply_demurrage(DEFAULT_DEMURRAGE, ONE_YEAR).principal, 0f64);
	}

	#[test]
	fn apply_demurrage_when_demurrage_is_negative_works() {
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);
		assert_abs_diff_eq(bal.apply_demurrage(-DEFAULT_DEMURRAGE, ONE_YEAR).principal, 0.5);
	}

	#[test]
	fn apply_demurrage_with_overflowing_values_works() {
		let demurrage = Demurrage::from_num(0.000048135220872218395);
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);

		// This produced a overflow before: https://github.com/encointer/encointer-node/issues/290
		assert_abs_diff_eq(bal.apply_demurrage(demurrage, ONE_YEAR).principal, 0f64);

		// Just make a ridiculous assumption.
		assert_abs_diff_eq(
			bal.apply_demurrage(Demurrage::from_num(u32::MAX), u32::MAX).principal,
			0f64,
		);
	}

	#[test]
	fn apply_demurrage_with_block_number_bigger_than_u32max_does_not_overflow() {
		let demurrage = Demurrage::from_num(DEFAULT_DEMURRAGE);
		let bal = BalanceEntry::<u64>::new(1u32.into(), 0);

		assert_abs_diff_eq(bal.apply_demurrage(demurrage, u32::MAX as u64 + 1).principal, 0f64);
	}

	#[test]
	fn apply_demurrage_with_block_number_not_monotonically_rising_just_updates_last_block() {
		let demurrage = Demurrage::from_num(DEFAULT_DEMURRAGE);
		let bal = BalanceEntry::<u32>::new(1u32.into(), 1);

		assert_eq!(bal.apply_demurrage(demurrage, 0), BalanceEntry::<u32>::new(1u32.into(), 0))
	}

	#[test]
	fn apply_demurrage_with_zero_demurrage_works() {
		let demurrage = Demurrage::from_num(0.0);
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);

		assert_abs_diff_eq(bal.apply_demurrage(demurrage, ONE_YEAR).principal, 1f64);
	}

	#[test]
	fn apply_demurrage_with_zero_elapsed_blocks_works() {
		let demurrage = Demurrage::from_num(DEFAULT_DEMURRAGE);
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);

		assert_abs_diff_eq(bal.apply_demurrage(demurrage, 0).principal, 1f64);
	}

	#[test]
	fn apply_demurrage_with_massive_demurrage_works() {
		// Have to do -1 otherwise we have a panic in the exp function:
		// https://github.com/encointer/substrate-fixed/issues/18
		//
		// Not critical as we safeguard against this with `validate_demurrage`.
		let demurrage = Demurrage::from_num(i64::MAX - 1);
		let bal = BalanceEntry::<u32>::new(1u32.into(), 0);

		assert_abs_diff_eq(bal.apply_demurrage(demurrage, 1).principal, 0f64);
	}

	#[rstest(
		balance,
		expected_result,
		case(0_000_000_100_000_000_000u128, 0.0000001),
		case(1_000_000_000_000_000_000u128, 1f64),
		case(0_100_000_000_000_000_000u128, 0.1),
		case(12_500_011_800_000_000u128, 0.012_500_011_8) // test for potential back conversion error: https://github.com/encointer/encointer-node/issues/200
	)]
	fn u128_to_balance_type_conversion_works(balance: u128, expected_result: f64) {
		let balance_type = EncointerBalanceConverter::convert;

		let res: f64 = balance_type(balance).lossy_into();
		assert_abs_diff_eq!(res, expected_result, epsilon = 1.0e-12);
	}

	#[test]
	fn u128_to_balance_type_conversion_does_not_overflow() {
		// this test was problematic in the beginning
		let balance_type = EncointerBalanceConverter::convert;

		let res: f64 = balance_type(123_456_000_000_000_000_000u128).lossy_into();
		assert_abs_diff_eq!(res, 123.456, epsilon = 1.0e-12);
	}

	#[rstest(
		balance,
		expected_result,
		case(1f64, 1_000_000_000_000_000_000u128),
		case(0.1f64, 0_100_000_000_000_000_000u128),
		case(123.456f64, 123_456_000_000_000_000_000u128)
	)]
	fn balance_type_to_u128_conversion_works(balance: f64, expected_result: u128) {
		let fungible = EncointerBalanceConverter::convert;

		let balance = BalanceType::from_num(balance);
		assert!(almost_eq(fungible(balance), expected_result, 10000));
	}
}
