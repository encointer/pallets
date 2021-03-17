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
    storage::{StorageDoubleMap, StorageMap},
};
use frame_system::ensure_signed;
use rstd::prelude::*;

use encointer_primitives::common::validate_ipfs_cid;
use encointer_primitives::{
    bazaar::{ArticleIdentifier, ShopIdentifier},
    communities::CommunityIdentifier,
};

pub trait Config: frame_system::Config + encointer_communities::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

decl_storage! {
    trait Store for Module<T: Config> as Bazaar {
        // Maps the shop or article owner to the respective items
        pub ShopsOwned get(fn shops_owned): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<ShopIdentifier>;
        pub ArticlesOwned get(fn articles_owned): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<ArticleIdentifier>;
        // Item owner
        pub ShopOwner get(fn shop_owner): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) ShopIdentifier => T::AccountId;
        pub ArticleOwner get(fn article_owner): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) ArticleIdentifier => (T::AccountId, ShopIdentifier);
        // The set of all shops and articles per community
        pub ShopRegistry get(fn shop_registry): map hasher(blake2_128_concat) CommunityIdentifier => Vec<ShopIdentifier>;
        pub ArticleRegistry get(fn article_registry): map hasher(blake2_128_concat) CommunityIdentifier => Vec<ArticleIdentifier>;
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
        /// Event emitted when a shop is uploaded. [community, who, shop]
        ShopCreated(CommunityIdentifier, AccountId, ShopIdentifier),
        /// Event emitted when a shop is removed by the owner. [community, who, shop]
        ShopRemoved(CommunityIdentifier, AccountId, ShopIdentifier),
    }
}

decl_error! {
    pub enum Error for Module<T: Config> {
        /// no such shop exisiting that could be deleted
        NoSuchShop,
        /// shop can not be created twice
        ShopAlreadyCreated,
        /// shop can not be removed by anyone else than its owner
        OnlyOwnerCanRemoveShop,
        /// invalid IpfsCid supplied
        InvalidIpfsCid,
    }
}

// TODO: Add Article Upload / Removal
decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;

        /// Allow a user to create a shop
        #[weight = 10_000]
        pub fn new_shop(origin, cid: CommunityIdentifier, shop: ShopIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;
            // Check that the supplied community is actually registered
            ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
                "CommunityIdentifier not found");

            let mut owned_shops = ShopsOwned::<T>::get(cid, &sender);
            let mut shops = ShopRegistry::get(cid);

            ensure!(validate_ipfs_cid(&shop).is_ok(), Error::<T>::InvalidIpfsCid);

            // Verify that the specified shop has not already been created with fast search
            ensure!(!ShopOwner::<T>::contains_key(cid, &shop), Error::<T>::ShopAlreadyCreated);

            // Add the shop to the registries
            owned_shops.push(shop.clone());
            shops.push(shop.clone());
            // Update blockchain
            ShopsOwned::<T>::insert(cid, &sender, owned_shops);
            ShopOwner::<T>::insert(cid, &shop, &sender);
            ShopRegistry::insert(cid, shops);
            // Emit an event that the shop was created
            Self::deposit_event(RawEvent::ShopCreated(cid, sender, shop));
            Ok(())
        }

        /// Allow a user to remove their shop
        #[weight = 10_000]
        pub fn remove_shop(origin, cid:CommunityIdentifier, shop: ShopIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;

            let mut owned_shops = ShopsOwned::<T>::get(cid, &sender);
            let mut shops = ShopRegistry::get(cid);

            // Verify that the removal request is coming from the righteous owner
            let shop_owner = ShopOwner::<T>::get(cid, &shop);
            ensure!(shop_owner == sender, Error::<T>::OnlyOwnerCanRemoveShop);

            // Get the index of the shop in the owner list
            match owned_shops.binary_search(&shop) {
                // Get the index of the shop registry
                Ok(shop_registry_index) => {
                    match owned_shops.binary_search(&shop) {
                        // If the search succeeds, delete the respective entries
                        Ok(onwed_shops_index) => {
                            // Remove the shop from the local registries
                            owned_shops.remove(onwed_shops_index);
                            shops.remove(shop_registry_index);
                            // Update blockchain
                            ShopsOwned::<T>::insert(cid, &sender, owned_shops);
                            ShopRegistry::insert(cid, shops);
                            ShopOwner::<T>::remove(cid, &shop);
                            // Emit an event that the shop was removed
                            Self::deposit_event(RawEvent::ShopRemoved(cid, sender, shop));
                            Ok(())
                        },
                        // If the search fails, no such shop is owned
                        Err(_) => Err(Error::<T>::NoSuchShop.into()),
                    }
                },
                // If the search fails, no such shop is owned
                Err(_) => Err(Error::<T>::NoSuchShop.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests;
