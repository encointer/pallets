use super::*;
use encointer_primitives::common::PalletString;
use frame_support::{
	inherent::Vec,
	traits::{
		fungibles::InspectMetadata,
		tokens::{DepositConsequence, WithdrawConsequence},
	},
};
use sp_runtime::{traits::Zero, DispatchError};

impl<T: Config> fungibles::InspectMetadata<T::AccountId> for Pallet<T> {
	fn name(asset: &Self::AssetId) -> Vec<u8> {
		PalletString::from("Encointer").into()
	}

	fn symbol(asset: &Self::AssetId) -> Vec<u8> {
		PalletString::from("ETR").into()
	}

	fn decimals(asset: &Self::AssetId) -> u8 {
		// Our BalanceType is I64F64, so the smallest possible number is 2^-64 = 5.42101086242752217003726400434970855712890625 × 10^-20
		//  and an upper bound is 2^63 + 1 = 9.223372036854775809 × 10^18
		// so we chose 18 decimals and lose some precision but can prevent overflows that way.
		18u8
	}
}

impl<T: Config> Pallet<T> {
	pub(crate) fn balance_type_to_fungible_balance(
		asset: <Pallet<T> as fungibles::Inspect<T::AccountId>>::AssetId,
		balance: BalanceType,
	) -> <Pallet<T> as fungibles::Inspect<T::AccountId>>::Balance {
		assert!(balance >= BalanceType::from_num(0));
		let decimals = Self::decimals(&asset);

		let bits = balance.to_bits();
		let mut result: u128 = 0;

		result = result + (bits >> 64) as u128 * 10u128.pow(decimals as u32);

		result = result +
			(BalanceType::from_bits((bits as i64) as i128) *
				BalanceType::from_num(10u128.pow(decimals as u32)))
			.to_num::<u128>();
		result
	}

	pub(crate) fn fungible_balance_to_balance_type(
		asset: <Pallet<T> as fungibles::Inspect<T::AccountId>>::AssetId,
		fungible_balance: <Pallet<T> as fungibles::Inspect<T::AccountId>>::Balance,
	) -> BalanceType {
		let decimals = Self::decimals(&asset);
		let mut result: BalanceType = BalanceType::from_num(0);

		result = result +
			BalanceType::from_num(
				((fungible_balance << 64) >> 64) as f64 * 10f64.powf(decimals as f64 * -1f64),
			);
		result = result +
			BalanceType::from_num(
				((fungible_balance >> 64) << 64) as f64 * 10f64.powf(decimals as f64 * -1f64),
			);
		result
	}
}

impl<T: Config> fungibles::Inspect<T::AccountId> for Pallet<T> {
	type AssetId = CommunityIdentifier;
	type Balance = u128;

	fn total_issuance(asset: Self::AssetId) -> Self::Balance {
		Self::balance_type_to_fungible_balance(asset, Pallet::<T>::total_issuance(asset))
	}

	fn minimum_balance(asset: Self::AssetId) -> Self::Balance {
		0
	}

	fn balance(asset: Self::AssetId, who: &T::AccountId) -> Self::Balance {
		Self::balance_type_to_fungible_balance(asset, Pallet::<T>::balance(asset, who))
	}

	fn reducible_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		keep_alive: bool,
	) -> Self::Balance {
		Self::balance_type_to_fungible_balance(asset, Pallet::<T>::balance(asset, who))
	}

	fn can_deposit(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DepositConsequence {
		if !<TotalIssuance<T>>::contains_key(asset) {
			return DepositConsequence::UnknownAsset
		};

		let total_issuance = Pallet::<T>::total_issuance_entry(asset).principal;

		let balance_amount = Self::fungible_balance_to_balance_type(asset, amount);
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
		if Self::balance_type_to_fungible_balance(asset, total_issuance.principal)
			.checked_sub(amount)
			.is_none()
		{
			return Underflow
		}

		if amount.is_zero() {
			return Success
		}

		let balance =
			Self::balance_type_to_fungible_balance(asset, Pallet::<T>::balance(asset, who));

		if balance.checked_sub(amount).is_none() {
			return NoFunds
		}
		Success
	}
}

impl<T: Config> fungibles::Unbalanced<T::AccountId> for Pallet<T> {
	fn set_balance(
		asset: Self::AssetId,
		who: &T::AccountId,
		amount: Self::Balance,
	) -> DispatchResult {
		let current_block = frame_system::Pallet::<T>::block_number();
		<Balance<T>>::insert(
			asset,
			who,
			BalanceEntry {
				principal: Self::fungible_balance_to_balance_type(asset, amount),
				last_update: current_block,
			},
		);
		Ok(())
	}

	fn set_total_issuance(asset: Self::AssetId, amount: Self::Balance) {
		let current_block = frame_system::Pallet::<T>::block_number();
		<TotalIssuance<T>>::insert(
			asset,
			BalanceEntry {
				principal: Self::fungible_balance_to_balance_type(asset, amount),
				last_update: current_block,
			},
		);
	}
}
