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

//! # Encointer Bazaar Module
//!
//! provides functionality for
//! - creating new bazaar entries (shop and articles)
//! - removing existing entries (shop and articles)
//!

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::ensure;
use frame_system::ensure_signed;
use sp_std::prelude::*;

use encointer_primitives::{
	bazaar::{BusinessData, BusinessIdentifier, OfferingData, OfferingIdentifier},
	common::PalletString,
	communities::CommunityIdentifier,
};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + encointer_communities::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_business(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			url: PalletString,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;
			// Check that the supplied community is actually registered
			ensure!(
				<encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::NonexistentCommunity
			);

			ensure!(
				!BusinessRegistry::<T>::contains_key(cid, &sender),
				Error::<T>::ExistingBusiness
			);

			BusinessRegistry::<T>::insert(cid, &sender, BusinessData::new(url, 1));

			Self::deposit_event(Event::BusinessCreated(cid, sender));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn update_business(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			url: PalletString,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;

			ensure!(
				BusinessRegistry::<T>::contains_key(cid, &sender),
				Error::<T>::NonexistentBusiness
			);

			BusinessRegistry::<T>::mutate(cid, &sender, |b| b.url = url);

			Self::deposit_event(Event::BusinessUpdated(cid, sender));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn delete_business(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;

			ensure!(
				BusinessRegistry::<T>::contains_key(cid, &sender),
				Error::<T>::NonexistentBusiness
			);

			BusinessRegistry::<T>::remove(cid, &sender);
			OfferingRegistry::<T>::remove_prefix(
				BusinessIdentifier::new(cid, sender.clone()),
				None,
			);

			Self::deposit_event(Event::BusinessDeleted(cid, sender));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn create_offering(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			url: PalletString,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;

			ensure!(
				BusinessRegistry::<T>::contains_key(cid, &sender),
				Error::<T>::NonexistentBusiness
			);

			let oid = BusinessRegistry::<T>::get(cid, &sender).last_oid;
			BusinessRegistry::<T>::mutate(cid, &sender, |b| b.last_oid = b.last_oid + 1);
			OfferingRegistry::<T>::insert(
				BusinessIdentifier::new(cid, sender.clone()),
				oid,
				OfferingData::new(url),
			);

			Self::deposit_event(Event::OfferingCreated(cid, sender, oid));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn update_offering(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			oid: OfferingIdentifier,
			url: PalletString,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;

			let business_identifier = BusinessIdentifier::new(cid, sender.clone());

			ensure!(
				OfferingRegistry::<T>::contains_key(&business_identifier, &oid),
				Error::<T>::NonexistentOffering
			);

			OfferingRegistry::<T>::mutate(business_identifier, oid, |o| o.url = url);

			Self::deposit_event(Event::OfferingUpdated(cid, sender, oid));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn delete_offering(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			oid: OfferingIdentifier,
		) -> DispatchResultWithPostInfo {
			// Check that the extrinsic was signed and get the signer
			let sender = ensure_signed(origin)?;

			let business_identifier = BusinessIdentifier::new(cid, sender.clone());

			ensure!(
				OfferingRegistry::<T>::contains_key(&business_identifier, &oid),
				Error::<T>::NonexistentOffering
			);

			OfferingRegistry::<T>::remove(business_identifier, oid);

			Self::deposit_event(Event::OfferingDeleted(cid, sender, oid));

			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Event emitted when a business is created. [community, who]
		BusinessCreated(CommunityIdentifier, T::AccountId),
		/// Event emitted when a business is updated. [community, who]
		BusinessUpdated(CommunityIdentifier, T::AccountId),
		/// Event emitted when a business is deleted. [community, who]
		BusinessDeleted(CommunityIdentifier, T::AccountId),
		/// Event emitted when an offering is created. [community, who, oid]
		OfferingCreated(CommunityIdentifier, T::AccountId, OfferingIdentifier),
		/// Event emitted when an offering is updated. [community, who, oid]
		OfferingUpdated(CommunityIdentifier, T::AccountId, OfferingIdentifier),
		/// Event emitted when an offering is deleted. [community, who, oid]
		OfferingDeleted(CommunityIdentifier, T::AccountId, OfferingIdentifier),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// community identifier not found
		NonexistentCommunity,
		/// business already registered for this cid
		ExistingBusiness,
		/// business does not exist
		NonexistentBusiness,
		/// offering does not exist
		NonexistentOffering,
	}

	#[pallet::storage]
	#[pallet::getter(fn business_registry)]
	pub type BusinessRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		BusinessData,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn offering_registry)]
	pub type OfferingRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		BusinessIdentifier<T::AccountId>,
		Blake2_128Concat,
		OfferingIdentifier,
		OfferingData,
		ValueQuery,
	>;
}

impl<T: Config> Pallet<T> {
	pub fn get_businesses(cid: &CommunityIdentifier) -> Vec<(T::AccountId, BusinessData)> {
		return BusinessRegistry::<T>::iter_prefix(cid).collect()
	}

	pub fn get_offerings(bid: &BusinessIdentifier<T::AccountId>) -> Vec<OfferingData> {
		return OfferingRegistry::<T>::iter_prefix_values(bid).collect()
	}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
