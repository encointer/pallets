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

use codec::Decode;
use core::marker::PhantomData;
use encointer_primitives::{
	communities::CommunityIdentifier, faucet::*, reputation_commitments::DescriptorType,
	scheduler::CeremonyIndexType,
};
use frame_support::{
	traits::{
		Currency,
		ExistenceRequirement::{AllowDeath, KeepAlive},
		Get, NamedReservableCurrency,
	},
	PalletId,
};
use frame_system::{self as frame_system, ensure_signed};
use log::info;
use sp_core::H256;
use sp_runtime::{traits::Hash, SaturatedConversion, Saturating};
use sp_std::convert::TryInto;
pub use weights::WeightInfo;

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod weights;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type ReserveIdentifierOf<T> = <<T as Config>::Currency as NamedReservableCurrency<
	<T as frame_system::Config>::AccountId,
>>::ReserveIdentifier;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ encointer_reputation_commitments::Config
		+ encointer_communities::Config
		+ pallet_treasury::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Currency<Self::AccountId> + NamedReservableCurrency<Self::AccountId>;
		type ControllerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type WeightInfo: WeightInfo;
		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
		T::AccountId: AsRef<[u8; 32]>,
		ReserveIdentifierOf<T>: From<[u8; 8]>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::create_faucet(), DispatchClass::Normal, Pays::Yes))]
		pub fn create_faucet(
			origin: OriginFor<T>,
			name: FaucetNameType,
			amount: BalanceOf<T>,
			whitelist: WhiteListType,
			drip_amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			let all_communities = encointer_communities::Pallet::<T>::community_identifiers();
			for cid in &whitelist {
				if !all_communities.contains(cid) {
					return Err(<Error<T>>::InvalidCommunityIdentifierInWhitelist.into())
				}
			}

			ensure!(
				drip_amount > <T as Config>::Currency::minimum_balance(),
				<Error<T>>::DripAmountTooSmall
			);

			// create account
			let faucet_identifier =
				[<T as Config>::PalletId::get().0.as_slice(), name.to_vec().as_slice()].concat();

			let faucet_id_hash: H256 = T::Hashing::hash_of(&faucet_identifier).into();
			let faucet_account = T::AccountId::decode(&mut faucet_id_hash.as_bytes())
				.expect("32 bytes can always construct an AccountId32");

			if <Faucets<T>>::contains_key(&faucet_account) {
				return Err(<Error<T>>::FaucetAlreadyExists.into())
			}

			<T as Config>::Currency::reserve_named(
				&Self::get_reserve_id(&faucet_account),
				&from,
				Self::reserve_amount(),
			)?;

			<T as Config>::Currency::transfer(&from, &faucet_account, amount, KeepAlive)
				.map_err(|_| <Error<T>>::InsuffiecientBalance)?;

			let purpose_id = <encointer_reputation_commitments::Pallet<T>>::do_register_purpose(
				DescriptorType::try_from(faucet_identifier)
					.map_err(|_| <Error<T>>::PurposeIdCreationFailed)?,
			)?;

			<Faucets<T>>::insert(
				&faucet_account,
				Faucet { name: name.clone(), purpose_id, whitelist, drip_amount, creator: from },
			);

			info!(target: LOG, "faucet created: {:?}, {:?}", name, faucet_account);
			Self::deposit_event(Event::FaucetCreated(faucet_account, name));

			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight((<T as Config>::WeightInfo::drip(), DispatchClass::Normal, Pays::Yes))]
		pub fn drip(
			origin: OriginFor<T>,
			faucet_account: T::AccountId,
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;

			let faucet = Self::faucets(&faucet_account).ok_or(<Error<T>>::InexsistentFaucet)?;

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

			<T as Config>::Currency::transfer(
				&faucet_account,
				&from,
				faucet.drip_amount,
				KeepAlive,
			)
			.map_err(|_| <Error<T>>::FaucetEmpty)?;

			info!(
				target: LOG,
				"faucet {:?} dripped {:?} to {:?}", faucet.name, faucet.drip_amount, from
			);
			Self::deposit_event(Event::Dripped(faucet_account, from, faucet.drip_amount));

			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::dissolve_faucet(), DispatchClass::Normal, Pays::Yes))]
		pub fn dissolve_faucet(
			origin: OriginFor<T>,
			faucet_account: T::AccountId,
			beneficiary: T::AccountId,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			let faucet = Self::faucets(&faucet_account).ok_or(<Error<T>>::InexsistentFaucet)?;

			<T as Config>::Currency::unreserve_all_named(
				&Self::get_reserve_id(&faucet_account),
				&faucet.creator,
			);

			<Faucets<T>>::remove(&faucet_account);
			<T as Config>::Currency::transfer(
				&faucet_account,
				&beneficiary,
				<T as Config>::Currency::free_balance(&faucet_account),
				AllowDeath,
			)?;

			info!(target: LOG, "faucet dissolved {:?}", faucet_account);
			Self::deposit_event(Event::FaucetDissolved(faucet_account));
			Ok(().into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight((<T as Config>::WeightInfo::close_faucet(), DispatchClass::Normal, Pays::Yes))]
		pub fn close_faucet(
			origin: OriginFor<T>,
			faucet_account: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let faucet = Self::faucets(&faucet_account).ok_or(<Error<T>>::InexsistentFaucet)?;

			ensure!(from == faucet.creator, <Error<T>>::NotCreator);
			ensure!(
				<T as Config>::Currency::free_balance(&faucet_account) <
					faucet.drip_amount.saturating_mul(2u32.saturated_into()),
				<Error<T>>::FaucetNotEmpty
			);

			<T as Config>::Currency::unreserve_all_named(
				&Self::get_reserve_id(&faucet_account),
				&faucet.creator,
			);

			<Faucets<T>>::remove(&faucet_account);
			<T as Config>::Currency::transfer(
				&faucet_account,
				&<pallet_treasury::Pallet<T>>::account_id(),
				<T as Config>::Currency::free_balance(&faucet_account),
				AllowDeath,
			)?;

			info!(target: LOG, "faucet closed {:?}", faucet_account);
			Self::deposit_event(Event::FaucetClosed(faucet_account));

			Ok(().into())
		}

		#[pallet::call_index(4)]
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

	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]>,
		ReserveIdentifierOf<T>: From<[u8; 8]>,
	{
		fn get_reserve_id(faucet_account: &T::AccountId) -> ReserveIdentifierOf<T> {
			let reserve_id: [u8; 8] = faucet_account.as_ref()[0..8]
				.try_into()
				.expect("[u8; 32] can always be converted to [u8; 8]");
			reserve_id.into()
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
		/// faucet dissolved
		FaucetDissolved(T::AccountId),
		/// faucet closed
		FaucetClosed(T::AccountId),
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
		/// facuet is not empty
		FaucetNotEmpty,
		/// sender is not faucet creator
		NotCreator,
		/// invalid community identifier in whitelist
		InvalidCommunityIdentifierInWhitelist,
		/// drip amount too small
		DripAmountTooSmall,
	}

	#[pallet::storage]
	#[pallet::getter(fn faucets)]
	pub(super) type Faucets<T: Config> =
		StorageMap<_, Identity, T::AccountId, Faucet<T::AccountId, BalanceOf<T>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn reserve_amount)]
	pub(super) type ReserveAmount<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;
}
