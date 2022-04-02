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

use crate::{AssetBalanceOf, AssetIdOf, BalanceOf};
use core::marker::PhantomData;
use encointer_primitives::{balances::EncointerBalanceConverter, communities::CommunityIdentifier};
use frame_support::traits::tokens::BalanceConversion;
use pallet_encointer_balances::Pallet as BalancesPallet;
use pallet_encointer_communities::{Config as CommunitiesConfig, Pallet as CommunitiesPallet};
use sp_runtime::traits::Convert;

/// Transforms the native token to the community currency
///
/// Assumptions:
/// * Native token has 12 decimals
/// * fee_conversion_factor is in Units 1 / [KSM]
pub fn balance_to_community_balance(
	balance: u128, // Unit = [pKSM]
	reward: u128,
	fee_conversion_factor: u32,
) -> u128 {
	return balance.saturating_mul(fee_conversion_factor as u128).saturating_mul(reward) /
		1_000_000_000_000 // <- unit discrepancy: balance [pKSM] vs. fee_conversion_factor [KSM]
}

pub struct BalanceToCommunityBalance<T>(PhantomData<T>);

impl<T> BalanceConversion<BalanceOf<T>, AssetIdOf<T>, AssetBalanceOf<T>>
	for BalanceToCommunityBalance<T>
where
	T: CommunitiesConfig + pallet_asset_tx_payment::Config,
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
		let reward = EncointerBalanceConverter::convert(CommunitiesPallet::<T>::nominal_income(
			CommunityIdentifier::from(asset_id),
		));

		Ok(balance_to_community_balance(balance.into(), reward, fee_conversion_factor).into())
	}
}
