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

use codec::{Decode, Encode};
use core::marker::PhantomData;
use encointer_primitives::{
	communities::CommunityIdentifier,
	faucet::*,
	reputation_commitments::{DescriptorType, PurposeIdType},
	scheduler::CeremonyIndexType,
};
use frame_support::{
	pallet_prelude::TypeInfo,
	traits::{
		Currency,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		Get,
	},
	PalletId, RuntimeDebug,
};
use frame_system::{self as frame_system, ensure_signed};
use log::info;
use sp_core::{MaxEncodedLen, H256};
use sp_runtime::traits::Hash;
use sp_std::convert::TryInto;

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn create_faucet(
			origin: OriginFor<T>,
			name: FaucetNameType,
			amount: BalanceOf<T>,
			whitelist: WhiteListType,
			drip_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			// create account
			let faucet_identifier =
				[T::PalletId::get().0.as_slice(), name.to_vec().as_slice()].concat();

			let faucet_id_hash: H256 = T::Hashing::hash_of(&faucet_identifier).into();
			let account_id = T::AccountId::decode(&mut faucet_id_hash.as_bytes())
				.expect("32 bytes can always construct an AccountId32");

			if <Faucets<T>>::contains_key(&account_id) {
				return Err(<Error<T>>::FaucetAlreadyExists.into())
			}

			T::Currency::transfer(&from, &account_id, amount, KeepAlive)
				.map_err(|_| <Error<T>>::InsuffiecientBalance)?;

			let purpose_id = <encointer_reputation_commitments::Pallet<T>>::do_register_purpose(
				DescriptorType::try_from(faucet_identifier)
					.map_err(|_| <Error<T>>::PurposeIdCreationFailed)?,
			)?;

			<Faucets<T>>::insert(
				&account_id,
				Faucet { name: name.clone(), purpose_id, whitelist, drip_amount },
			);

			Self::deposit_event(Event::FaucetCreated(account_id, name));

			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn drip(
			origin: OriginFor<T>,
			faucet_account: T::AccountId,
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			if !<Faucets<T>>::contains_key(&faucet_account) {
				return Err(<Error<T>>::InexsistentFaucet.into())
			}

			let faucet = Self::faucets(&faucet_account);

			if !faucet.whitelist.contains(&cid) {
				return Err(<Error<T>>::CommunityNotInWhitelist.into())
			}

			<encointer_reputation_commitments::Pallet<T>>::do_commit_reputation(
				&from,
				cid,
				cindex,
				faucet.purpose_id,
				None,
			)?;

			T::Currency::transfer(&faucet_account, &from, faucet.drip_amount, KeepAlive)
				.map_err(|_| <Error<T>>::FaucetEmpty)?;

			Self::deposit_event(Event::Dripped(faucet_account, from, faucet.drip_amount));

			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn dissolve_faucet(
			origin: OriginFor<T>,
			faucet_account: T::AccountId,
			beneficiary: T::AccountId,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			if !<Faucets<T>>::contains_key(&faucet_account) {
				return Err(<Error<T>>::InexsistentFaucet.into())
			}

			<Faucets<T>>::remove(&faucet_account);
			T::Currency::transfer(
				&faucet_account,
				&beneficiary,
				T::Currency::free_balance(&faucet_account),
				AllowDeath,
			)?;

			Ok(().into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn set_reserve_amount(
			origin: OriginFor<T>,
			reserve_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			T::ControllerOrigin::ensure_origin(origin)?;
			<ReserveAmount<T>>::put(reserve_amount);
			info!(target: LOG, "reserve amount set to {:?} s", reserve_amount);
			Self::deposit_event(Event::ReserveAmountUpdated(reserve_amount));
			Ok(().into())
		}
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub reserve_amount: BalanceOf<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { reserve_amount: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			<ReserveAmount<T>>::put(self.reserve_amount);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// faucet dripped | facuet account, receiver account, balance
		Dripped(T::AccountId, T::AccountId, BalanceOf<T>),
		/// faucet created
		FaucetCreated(T::AccountId, FaucetNameType),
		/// reserve amount updated
		ReserveAmountUpdated(BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// faucet is empty
		FaucetEmpty,
		/// insufficient balance to create the faucet
		InsuffiecientBalance,
		/// faucet already exists
		FaucetAlreadyExists,
		/// faucet does not exist
		InexsistentFaucet,
		/// purposeId creation failed
		PurposeIdCreationFailed,
		/// cid not in whitelist
		CommunityNotInWhitelist,
	}

	#[pallet::storage]
	#[pallet::getter(fn faucets)]
	pub(super) type Faucets<T: Config> =
		StorageMap<_, Identity, T::AccountId, Faucet<BalanceOf<T>>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn reserve_amount)]
	pub(super) type ReserveAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;
}
