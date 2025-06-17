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

#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use encointer_primitives::{balances::BalanceType, communities::CommunityIdentifier};
use frame_support::{
	traits::{Currency, ExistenceRequirement::KeepAlive, Get},
	PalletId,
};
use frame_system::ensure_signed;
use log::info;
use parity_scale_codec::Decode;
use sp_core::H256;
use sp_runtime::traits::Hash;
// Logger target
const LOG: &str = "encointer";

pub use crate::weights::WeightInfo;
pub use pallet::*;
pub use transfer::Transfer;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod transfer;
mod weights;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use encointer_primitives::treasuries::{SwapAssetOption, SwapNativeOption};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_encointer_balances::Config + pallet_timestamp::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Currency<Self::AccountId>;

		/// The treasuries' pallet id, used for deriving sovereign account IDs per community.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		// /// the maximum fraction of available treasury funds a single swap can claim
		// /// defined as divisor: 2 means half of the available funds can be swapped
		// #[pallet::constant]
		// type MaxFractionPerSwap: Get<u8>;
		//
		// /// the minimum period an account has to wait between two swaps
		// #[pallet::constant]
		// type SwapCooldownPeriod: Get<T::Moment>;

		/// Type parameter representing the asset kinds to be spent from the treasury.
		/// This can be the unit type if only native is supported.
		type AssetKind: Parameter + MaxEncodedLen;

		/// Type for processing spends of [Self::AssetKind] in favor of [`Self::Beneficiary`].
		type Paymaster: Transfer<
			Payer = Self::AccountId,
			Beneficiary = Self::AccountId,
			AssetKind = Self::AssetKind,
			Balance = BalanceOf<Self>,
		>;

		/// Helper type for benchmarks.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::benchmarking::ArgumentsFactory<Self::AssetKind>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn swap_native_options)]
	pub type SwapNativeOptions<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		SwapNativeOption<BalanceOf<T>, T::Moment>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn swap_asset_options)]
	pub type SwapAssetOptions<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		SwapAssetOption<BalanceOf<T>, T::Moment, T::AssetKind>,
		OptionQuery,
	>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
	{
		/// swap native tokens for community currency subject to an existing swap option for the
		/// sender account.
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::swap_native(), DispatchClass::Normal, Pays::Yes))]
		pub fn swap_native(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			desired_native_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let swap_option =
				Self::swap_native_options(cid, &sender).ok_or(<Error<T>>::NoValidSwapOption)?;
			ensure!(
				swap_option.native_allowance >= desired_native_amount,
				Error::<T>::InsufficientAllowance
			);
			let treasury_account = Self::get_community_treasury_account_unchecked(Some(cid));
			ensure!(
				T::Currency::free_balance(&treasury_account) - T::Currency::minimum_balance() >=
					desired_native_amount,
				Error::<T>::InsufficientNativeFunds
			);
			let rate = swap_option.rate.ok_or(Error::<T>::SwapRateNotDefined)?;
			let cc_amount = Self::cc_amount(desired_native_amount, rate)?;

			// Useful for debugging in tests. Enable if desired.
			// println!("Swapping => {cc_amount:?} CC => {desired_native_amount:?}  pKSM");
			if swap_option.do_burn {
				<pallet_encointer_balances::Pallet<T>>::burn(cid, &sender, cc_amount)?;
			} else {
				<pallet_encointer_balances::Pallet<T>>::do_transfer(
					cid,
					&sender,
					&treasury_account,
					cc_amount,
				)?;
			}

			let new_swap_option = SwapNativeOption {
				native_allowance: swap_option.native_allowance - desired_native_amount,
				..swap_option
			};
			<SwapNativeOptions<T>>::insert(cid, &sender, new_swap_option);
			Self::do_spend_native(Some(cid), &sender, desired_native_amount)?;
			Ok(().into())
		}

		/// swap native tokens for community currency subject to an existing swap option for the
		/// sender account.
		#[pallet::call_index(1)]
		#[pallet::weight((<T as Config>::WeightInfo::swap_asset(), DispatchClass::Normal, Pays::Yes))]
		pub fn swap_asset(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			desired_asset_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let swap_option =
				Self::swap_asset_options(cid, &sender).ok_or(<Error<T>>::NoValidSwapOption)?;
			ensure!(
				swap_option.asset_allowance >= desired_asset_amount,
				Error::<T>::InsufficientAllowance
			);

			// Note: We have no means of checking the treasury balance as it lives on another chain.
			let treasury_account = Self::get_community_treasury_account_unchecked(Some(cid));

			let rate = swap_option.rate.ok_or(Error::<T>::SwapRateNotDefined)?;
			let cc_amount = Self::cc_amount(desired_asset_amount, rate)?;

			// Useful for debugging in tests. Enable if desired.
			// println!("Swapping => {cc_amount:?} CC => {desired_native_amount:?}  pKSM");
			if swap_option.do_burn {
				<pallet_encointer_balances::Pallet<T>>::burn(cid, &sender, cc_amount)?;
			} else {
				<pallet_encointer_balances::Pallet<T>>::do_transfer(
					cid,
					&sender,
					&treasury_account,
					cc_amount,
				)?;
			}

			let new_swap_option = SwapAssetOption {
				asset_allowance: swap_option.asset_allowance - desired_asset_amount,
				..swap_option
			};
			<SwapAssetOptions<T>>::insert(cid, &sender, &new_swap_option);
			Self::do_spend_asset(
				Some(cid),
				&sender,
				new_swap_option.asset_id,
				desired_asset_amount,
			)?;
			Ok(().into())
		}

		/// Only used for testing
		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::swap_asset(), DispatchClass::Normal, Pays::Yes))]
		pub fn test_asset_pay(
			origin: OriginFor<T>,
			cid: Option<CommunityIdentifier>,
			asset_id: Box<T::AssetKind>,
			desired_asset_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			Self::do_spend_asset(cid, &sender, *asset_id, desired_asset_amount)?;
			Ok(().into())
		}
	}
	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
	{
		pub fn get_community_treasury_account_unchecked(
			maybecid: Option<CommunityIdentifier>,
		) -> T::AccountId {
			let treasury_identifier =
				[<T as Config>::PalletId::get().0.as_slice(), maybecid.encode().as_slice()]
					.concat();
			let treasury_id_hash: H256 = T::Hashing::hash_of(&treasury_identifier).into();
			T::AccountId::decode(&mut treasury_id_hash.as_bytes())
				.expect("32 bytes can always construct an AccountId32")
		}

		pub fn cc_amount(
			desired_native_amount: BalanceOf<T>,
			rate: BalanceType,
		) -> Result<BalanceType, Error<T>> {
			let native_u64: u64 =
				desired_native_amount.try_into().or(Err(Error::<T>::SwapOverflow))?;

			BalanceType::from_num::<u64>(native_u64)
				.checked_mul(rate)
				.ok_or(Error::<T>::SwapOverflow)
		}

		pub fn do_spend_native(
			maybe_cid: Option<CommunityIdentifier>,
			beneficiary: &T::AccountId,
			amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let treasury = Self::get_community_treasury_account_unchecked(maybe_cid);
			T::Currency::transfer(&treasury, beneficiary, amount, KeepAlive)?;
			info!(target: LOG, "treasury spent native: {:?}, {:?} to {:?}", maybe_cid, amount, beneficiary);
			Self::deposit_event(Event::SpentNative {
				treasury,
				beneficiary: beneficiary.clone(),
				amount,
			});
			Ok(().into())
		}

		pub fn do_spend_asset(
			maybe_cid: Option<CommunityIdentifier>,
			beneficiary: &T::AccountId,
			asset_id: T::AssetKind,
			amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let treasury = Self::get_community_treasury_account_unchecked(maybe_cid);
			T::Paymaster::transfer(&treasury, beneficiary, asset_id.clone(), amount).map_err(
				|e| {
					log::error!(target: LOG, "Paymaster payout error: {:?}", e);
					Error::<T>::PayoutError
				},
			)?;
			info!(target: LOG, "treasury spent native: {:?}, {:?} to {:?}", maybe_cid, amount, beneficiary);
			Self::deposit_event(Event::SpentAsset {
				treasury,
				beneficiary: beneficiary.clone(),
				asset_id,
				amount,
			});
			Ok(().into())
		}

		/// store a swap option possibly replacing any previously existing option
		pub fn do_issue_swap_native_option(
			cid: CommunityIdentifier,
			who: &T::AccountId,
			option: SwapNativeOption<BalanceOf<T>, T::Moment>,
		) -> DispatchResultWithPostInfo {
			SwapNativeOptions::<T>::insert(cid, who, option);
			Self::deposit_event(Event::GrantedSwapNativeOption { cid, who: who.clone() });
			Ok(().into())
		}

		/// store a swap option possibly replacing any previously existing option
		pub fn do_issue_swap_asset_option(
			cid: CommunityIdentifier,
			who: &T::AccountId,
			option: SwapAssetOption<BalanceOf<T>, T::Moment, T::AssetKind>,
		) -> DispatchResultWithPostInfo {
			SwapAssetOptions::<T>::insert(cid, who, option);
			Self::deposit_event(Event::GrantedSwapNativeOption { cid, who: who.clone() });
			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub (super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// treasury spent native tokens from community `cid` to `beneficiary` amounting `amount`
		SpentNative {
			treasury: T::AccountId,
			beneficiary: T::AccountId,
			amount: BalanceOf<T>,
		},
		SpentAsset {
			treasury: T::AccountId,
			beneficiary: T::AccountId,
			asset_id: T::AssetKind,
			amount: BalanceOf<T>,
		},
		GrantedSwapNativeOption {
			cid: CommunityIdentifier,
			who: T::AccountId,
		},
		GrantedSwapAssetOption {
			cid: CommunityIdentifier,
			who: T::AccountId,
			asset_id: T::AssetKind,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// no valid swap option. Either no option at all or insufficient allowance
		NoValidSwapOption,
		SwapRateNotDefined,
		SwapOverflow,
		InsufficientNativeFunds,
		InsufficientAllowance,
		PayoutError,
	}
}
