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
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Convert;

#[cfg(feature = "serde_derive")]
use ep_core::serde::serialize_fixed;

// We're working with fixpoint here.

/// Encointer balances are fixpoint values
pub type BalanceType = I64F64;

/// Demurrage is the rate of evanescence of balances per block
/// it must be positive
/// 0.0 : no demurrage at all
/// 1.3188e-07 : halving time of 1 year if blocktime is 6s
pub type Demurrage = I64F64;

pub type FeeConversionFactorType = u32;

#[derive(
	Encode, Decode, Default, RuntimeDebug, Clone, Copy, PartialEq, TypeInfo, MaxEncodedLen,
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

/// Our BalanceType is I64F64, so the smallest possible number is
/// 2^-64 = 5.42101086242752217003726400434970855712890625 × 10^-20
/// and the upper bound is 2^63 + 1 = 9.223372036854775809 × 10^18
///
/// We choose 18 decimals and lose some precision, but can prevent overflows that way.
pub const ENCOINTER_BALANCE_DECIMALS: u32 = 18;

pub struct EncointerBalanceConverter;

// Todo: Make u128 generic
impl Convert<BalanceType, u128> for EncointerBalanceConverter {
	fn convert(balance: BalanceType) -> u128 {
		let decimals = ENCOINTER_BALANCE_DECIMALS;

		let bits = balance.to_bits();
		let mut result: u128 = 0;

		result += (bits >> 64) as u128 * 10u128.pow(decimals);

		result += (BalanceType::from_bits((bits as i64) as i128) * // <- to truncate
			BalanceType::from_num(10u128.pow(decimals)))
		.to_num::<u128>();
		result
	}
}

impl Convert<u128, BalanceType> for EncointerBalanceConverter {
	fn convert(fungible_balance: u128) -> BalanceType {
		let decimals = ENCOINTER_BALANCE_DECIMALS;

		let mut result: BalanceType = BalanceType::from_num(0);

		result += BalanceType::from_num(
			((fungible_balance << 64) >> 64) as f64 / (10i128.pow(decimals as u32) as f64),
		);

		result += BalanceType::from_num(fungible_balance >> 64) *
			BalanceType::from_num(2i128.pow(64) as f64 / 10i128.pow(decimals as u32) as f64);

		result
	}
}
