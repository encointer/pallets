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

use crate::{AccountIdOf, AssetBalanceOf, AssetIdOf, BalanceOf, FungiblesOf};
use core::marker::PhantomData;
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::traits::{fungibles, tokens::BalanceConversion};
use pallet_encointer_balances::Pallet as BalancesPallet;
use pallet_encointer_communities::{Config as CommunitiesConfig, Pallet as CommunitiesPallet};

pub fn balance_to_community_balance<T: CommunitiesConfig>(
	balance: u128,
	cid: CommunityIdentifier,
	reward: u128,
	fee_conversion_factor: u32,
	asset_balance_decimals: u8,
) -> u128 {
	// 5.233 micro ksm correspond to 0.01 units of the community currency assuming a feeConversionFactor of 10_000
	// the KSM balance parameter comes with 12 decimals
	// 5.233 * 10^6 pKSM = 0.01 * 10^decimals LEU
	// 5.233 * 10^6 pKSM = 0.01 * 10^(decimals - 4) * feeConversionFactor LEU
	// 1 pKSM = (0.01 * 10^(decimals - 4) * feeConversionFactor) / (5.233 * 10^6) LEU
	// 1 pKSM = (0.01 * 10^(decimals - 10) * feeConversionFactor) / 5.233 LEU
	let conversion_factor = ((0.01f64 / 5.233f64) *
		10i128.pow((asset_balance_decimals - 10) as u32) as f64 *
		fee_conversion_factor as f64) as u128;

	// assuming a nominal income of 20
	return (balance * conversion_factor * reward) /
		BalancesPallet::<T>::balance_type_to_fungible_balance(
			cid.into(),
			BalanceType::from_num(20i32),
		)
}

pub struct BalanceToCommunityBalance<T>(PhantomData<T>);

impl<T> BalanceConversion<BalanceOf<T>, AssetIdOf<T>, AssetBalanceOf<T>>
	for BalanceToCommunityBalance<T>
where
	T: CommunitiesConfig + pallet_asset_tx_payment::Config,
	CommunityIdentifier: From<AssetIdOf<T>>,
	AssetBalanceOf<T>: From<u128>,
	FungiblesOf<T>: fungibles::InspectMetadata<AccountIdOf<T>>,
	u128: From<BalanceOf<T>>,
{
	type Error = frame_system::Error<T>;

	fn to_asset_balance(
		balance: BalanceOf<T>,
		asset_id: AssetIdOf<T>,
	) -> Result<AssetBalanceOf<T>, Self::Error> {
		let decimals = <FungiblesOf<T> as fungibles::InspectMetadata<AccountIdOf<T>>>::decimals(
			&asset_id.into(),
		);

		let fee_conversion_factor = BalancesPallet::<T>::fee_conversion_factor();
		let reward = BalancesPallet::<T>::balance_type_to_fungible_balance(
			asset_id.into(),
			CommunitiesPallet::<T>::nominal_income(CommunityIdentifier::from(asset_id)),
		);
		let balance_u128: u128 = balance.into();

		Ok(balance_to_community_balance::<T>(
			balance_u128,
			CommunityIdentifier::from(asset_id),
			reward,
			fee_conversion_factor,
			decimals,
		)
		.into())
	}
}
