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

use super::*;
use encointer_primitives::balances::EncointerBalanceConverter;
use frame_support::traits::tokens::{
	DepositConsequence, Fortitude, Preservation, Provenance, WithdrawConsequence,
};
use sp_runtime::traits::{Convert, Zero};

pub(crate) fn fungible(balance: BalanceType) -> u128 {
	EncointerBalanceConverter::convert(balance)
}

pub(crate) fn balance_type(fungible: u128) -> BalanceType {
	EncointerBalanceConverter::convert(fungible)
}

impl<T: Config> fungibles::Inspect<T::AccountId> for Pallet<T> {
	type AssetId = CommunityIdentifier;
	type Balance = u128;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		fungible(Pallet::<T>::total_issuance(asset))
	}

	fn asset_exists(asset: Self::AssetId) -> bool {
		Pallet::<T>::total_issuance(asset) > 0
	}

	fn minimum_balance(_asset: Self::AssetId) -> Self::Balance {
		0
	}

	fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		fungible(Pallet::<T>::balance(asset, who))
	}

	fn total_balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		fungible(Pallet::<T>::balance(asset, who))
	}

	fn reducible_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		_preservation: Preservation,
		_force: Fortitude,
	) -> Self::Balance {
		fungible(Pallet::<T>::balance(asset, who))
	}

	fn can_deposit(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
		_mint: Provenance,
	) -> DepositConsequence {
		if !<TotalIssuance<T>>::contains_key(asset) {
			return DepositConsequence::UnknownAsset
		};

		let total_issuance = Pallet::<T>::total_issuance_entry(asset).principal;

		let balance_amount = balance_type(amount);
		if total_issuance.checked_add(balance_amount).is_none() {
			return DepositConsequence::Overflow
		}

		let balance = Pallet::<T>::balance(asset, who);

		if balance.checked_add(balance_amount).is_none() {
			return DepositConsequence::Overflow
		}

		DepositConsequence::Success
	}

	fn can_withdraw(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> WithdrawConsequence<Self::Balance> {
		use WithdrawConsequence::*;

		if !<TotalIssuance<T>>::contains_key(asset) {
			return UnknownAsset
		};

		let total_issuance = Pallet::<T>::total_issuance_entry(asset);
		if fungible(total_issuance.principal).checked_sub(amount).is_none() {
			return Underflow
		}

		if amount.is_zero() {
			return Success
		}

		let balance = fungible(Pallet::<T>::balance(asset, who));

		if balance.checked_sub(amount).is_none() {
			return WithdrawConsequence::BalanceLow
		}
		WithdrawConsequence::Success
	}
}

impl<T: Config> fungibles::Unbalanced<T::AccountId> for Pallet<T> {
	fn write_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> Result<Option<Self::Balance>, sp_runtime::DispatchError> {
		let current_block = frame_system::Pallet::<T>::block_number();
		<Balance<T>>::insert(
			asset,
			who,
			BalanceEntry { principal: balance_type(amount), last_update: current_block },
		);
		Ok(None)
	}

	fn set_total_issuance(asset: Self::AssetId, amount: Self::Balance) {
		let current_block = frame_system::Pallet::<T>::block_number();
		<TotalIssuance<T>>::insert(
			asset,
			BalanceEntry { principal: balance_type(amount), last_update: current_block },
		);
	}
	fn handle_dust(_: fungibles::Dust<T::AccountId, Self>) {}
}
