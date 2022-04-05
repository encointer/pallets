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

use crate::{AssetBalanceOf, AssetIdOf, BalanceOf, LOG};
use core::marker::PhantomData;
use encointer_primitives::{balances::EncointerBalanceConverter, communities::CommunityIdentifier};
use frame_support::traits::tokens::BalanceConversion;
use pallet_encointer_balances::Pallet as BalancesPallet;
use pallet_encointer_ceremonies::{Config as CeremoniesConfig, Pallet as CeremoniesPallet};
use sp_runtime::traits::Convert;

/// 1 micro KSM with 12 decimals
pub const ONE_MICRO_KSM: u128 = 1_000_000;

/// 1 KSM with 12 decimals
pub const ONE_KSM: u128 = 1_000_000 * ONE_MICRO_KSM;

/// 1 Kilo-KSM with 12 decimals
pub const ONE_KILO_KSM: u128 = 1_000 * ONE_KSM;

/// Transforms the native token to the community currency
///
/// Assumptions:
/// * Native token has 12 decimals
/// * fee_conversion_factor is in Units 1 / [pKSM]
///
/// Applies the formula: Community Currency = KSM * FeeConversionFactor * Reward
pub fn apply_fee_conversion_factor(
	balance: u128,
	reward: u128,
	fee_conversion_factor: u128,
) -> u128 {
	return balance
		.saturating_mul(reward)
		.saturating_mul(fee_conversion_factor as u128)
		.checked_div(ONE_KILO_KSM) // <- unit discrepancy: balance [pKSM] vs. fee_conversion_factor [KKSM]
		.expect("Divisor != 0; qed")
}

pub struct BalanceToCommunityBalance<T>(PhantomData<T>);

impl<T> BalanceConversion<BalanceOf<T>, AssetIdOf<T>, AssetBalanceOf<T>>
	for BalanceToCommunityBalance<T>
where
	T: CeremoniesConfig + pallet_asset_tx_payment::Config,
	CommunityIdentifier: From<AssetIdOf<T>>,
	AssetBalanceOf<T>: From<u128>,
	u128: From<BalanceOf<T>>,
{
	type Error = frame_system::Error<T>;

	fn to_asset_balance(
		balance: BalanceOf<T>,
		asset_id: AssetIdOf<T>,
	) -> Result<AssetBalanceOf<T>, Self::Error> {
		let fee_conversion_factor = BalancesPallet::<T>::fee_conversion_factor();

		let reward_balance_type =
			CeremoniesPallet::<T>::nominal_income(&CommunityIdentifier::from(asset_id));
		let reward = EncointerBalanceConverter::convert(reward_balance_type);

		let asset_fee =
			apply_fee_conversion_factor(balance.into(), reward, fee_conversion_factor).into();

		Ok(asset_fee)
	}
}
