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

use encointer_primitives::{
	communities::CommunityIdentifier,
	reputation_commitments::{DescriptorType, FromStr, PurposeIdType},
	scheduler::CeremonyIndexType,
};

use core::marker::PhantomData;
use frame_support::{
	traits::{Currency, ExistenceRequirement::KeepAlive, Get},
	PalletId,
};
use frame_system::{self as frame_system, ensure_signed};
use log::info;
use sp_runtime::{traits::AccountIdConversion, Saturating};
use sp_std::convert::TryInto;

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config + encointer_reputation_commitments::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Currency<Self::AccountId>;
		type ControllerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Transfer some balance to another account.
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn drip(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			<encointer_reputation_commitments::Pallet<T>>::do_commit_reputation(
				&from,
				cid,
				cindex,
				Self::reputation_commitments_purpose_id(),
				None,
			)?;

			T::Currency::transfer(&Self::account_id(), &from, Self::drip_amount(), KeepAlive)
				.map_err(|_| <Error<T>>::FaucetEmpty)?;

			Self::deposit_event(Event::Dripped(from, Self::drip_amount()));

			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn set_drip_amount(
			origin: OriginFor<T>,
			drip_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			T::ControllerOrigin::ensure_origin(origin)?;
			<DripAmount<T>>::put(drip_amount);
			info!(target: LOG, "set drip amount to {:?}", drip_amount);
			Self::deposit_event(Event::DripAmountUpdated(drip_amount));
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account ID of the faucet.
		///
		/// This actually does computation. If you need to keep using it, then make sure you cache the
		/// value and only call this once.
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		pub fn pot() -> BalanceOf<T> {
			T::Currency::free_balance(&Self::account_id())
				// Must never be less than 0 but better be safe.
				.saturating_sub(T::Currency::minimum_balance())
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub drip_amount: BalanceOf<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { drip_amount: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			// Create faucet account
			let account_id = <Pallet<T>>::account_id();
			let min = T::Currency::minimum_balance();
			if T::Currency::free_balance(&account_id) < min {
				let _ = T::Currency::make_free_balance_be(&account_id, min);
			}
			<DripAmount<T>>::put(self.drip_amount);

			let purpose_id = <encointer_reputation_commitments::Pallet<T>>::do_register_purpose(
				DescriptorType::from_str("EncointerFaucet").unwrap(),
			)
			.expect("In case of purpose registry overflow, we cannot use this pallet.");
			<ReputationCommitmentsPurposeId<T>>::put(purpose_id);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// faucet dripped
		Dripped(T::AccountId, BalanceOf<T>),
		/// drip amount updated
		DripAmountUpdated(BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		// faucet is empty
		FaucetEmpty,
	}

	#[pallet::storage]
	#[pallet::getter(fn drip_amount)]
	pub(super) type DripAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn reputation_commitments_purpose_id)]
	pub(super) type ReputationCommitmentsPurposeId<T: Config> =
		StorageValue<_, PurposeIdType, ValueQuery>;
}
