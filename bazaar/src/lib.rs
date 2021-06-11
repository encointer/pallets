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

#[cfg(test)]
extern crate approx;

use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    storage::{StorageDoubleMap},
};
use frame_system::ensure_signed;
use rstd::prelude::*;

use encointer_primitives::{
    bazaar::{BusinessIdentifier, BusinessData, OfferingData, OfferingIdentifier},
    communities::CommunityIdentifier,
    common::PalletString,
};

pub trait Config: frame_system::Config + encointer_communities::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
    trait Store for Module<T: Config> as Bazaar {
        pub BusinessRegistry get(fn business_registry): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) T::AccountId => BusinessData;
        pub OfferingRegistry get(fn offering_registry): double_map hasher(blake2_128_concat) BusinessIdentifier<T::AccountId>, hasher(blake2_128_concat) OfferingIdentifier => OfferingData;
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
        /// Event emitted when a business is created. [community, who]
        BusinessCreated(CommunityIdentifier, AccountId),
        /// Event emitted when a business is updated. [community, who]
        BusinessUpdated(CommunityIdentifier, AccountId),
        /// Event emitted when a business is deleted. [community, who]
        BusinessDeleted(CommunityIdentifier, AccountId),
        /// Event emitted when an offering is created. [community, who, oid]
        OfferingCreated(CommunityIdentifier, AccountId, OfferingIdentifier),
        /// Event emitted when an offering is updated. [community, who, oid]
        OfferingUpdated(CommunityIdentifier, AccountId, OfferingIdentifier),
        /// Event emitted when an offering is deleted. [community, who, oid]
        OfferingDeleted(CommunityIdentifier, AccountId, OfferingIdentifier),
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// community identifier not found
        InexistentCommunity,
        /// business already registered for this cid
        ExistingBusiness,
        /// business does not exist
        InexistentBusiness,
        /// offering does not exist
        InexistentOffering
    }
}

// TODO: Add Article Upload / Removal
decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

        #[weight = 10_000]
        pub fn create_business(origin, cid: CommunityIdentifier, url: PalletString) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            ensure!(!BusinessRegistry::<T>::contains_key(cid, sender.clone()), Error::<T>::ExistingBusiness);

            BusinessRegistry::<T>::insert(cid, sender.clone(), BusinessData { url: url, last_oid: 1 });

            Self::deposit_event(RawEvent::BusinessCreated(cid, sender));

            Ok(())
        }

        #[weight = 10_000]
        pub fn update_business(origin, cid: CommunityIdentifier, url: PalletString) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            ensure!(BusinessRegistry::<T>::contains_key(cid, sender.clone()), Error::<T>::InexistentBusiness);

            BusinessRegistry::<T>::mutate(cid, sender.clone(), |b| b.url = url);

            Self::deposit_event(RawEvent::BusinessUpdated(cid, sender));

            Ok(())
        }

        #[weight = 10_000]
        pub fn delete_business(origin, cid: CommunityIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            ensure!(BusinessRegistry::<T>::contains_key(cid, sender.clone()), Error::<T>::InexistentBusiness);

            BusinessRegistry::<T>::remove(cid, sender.clone());
            OfferingRegistry::<T>::remove_prefix(BusinessIdentifier{community_identifier: cid, business_account: sender.clone()});

            Self::deposit_event(RawEvent::BusinessDeleted(cid, sender));

            Ok(())
        }

        #[weight = 10_000]
        pub fn create_offering(origin, cid: CommunityIdentifier, url: PalletString) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            ensure!(BusinessRegistry::<T>::contains_key(cid, sender.clone()), Error::<T>::InexistentBusiness);

            let oid = BusinessRegistry::<T>::get(cid, sender.clone()).last_oid;
            BusinessRegistry::<T>::mutate(cid, sender.clone(), |b| b.last_oid = b.last_oid + 1);
            OfferingRegistry::<T>::insert(BusinessIdentifier{community_identifier: cid, business_account: sender.clone()}, oid, OfferingData{url});

            Self::deposit_event(RawEvent::OfferingCreated(cid, sender, oid));

            Ok(())
        }

        #[weight = 10_000]
        pub fn update_offering(origin, cid: CommunityIdentifier, oid: OfferingIdentifier, url: PalletString) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            let business_identifier = BusinessIdentifier{community_identifier: cid, business_account: sender.clone()};

            ensure!(OfferingRegistry::<T>::contains_key(business_identifier.clone(), oid.clone()), Error::<T>::InexistentOffering);

            OfferingRegistry::<T>::mutate(business_identifier, oid, |o| o.url = url);

            Self::deposit_event(RawEvent::OfferingUpdated(cid, sender, oid));

            Ok(())
        }

        #[weight = 10_000]
        pub fn delete_offering(origin, cid: CommunityIdentifier, oid: OfferingIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                Error::<T>::InexistentCommunity);

            let business_identifier = BusinessIdentifier{community_identifier: cid, business_account: sender.clone()};

            ensure!(OfferingRegistry::<T>::contains_key(business_identifier.clone(), oid.clone()), Error::<T>::InexistentOffering);

            OfferingRegistry::<T>::remove(business_identifier, oid);

            Self::deposit_event(RawEvent::OfferingDeleted(cid, sender, oid));

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
