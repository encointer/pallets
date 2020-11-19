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

use frame_support::{
    decl_event, decl_module, decl_storage, decl_error,
    dispatch::DispatchResult,
    ensure,
    //storage::StorageValue,
    //traits::Get,
    weights::{DispatchClass, Pays},
    debug
    //StorageMap nötig?
};
use sp_core::RuntimeDebug;
use frame_system::ensure_signed;
use sp_timestamp::OnTimestampSet;
//use rstd::prelude::*;
use codec::{Decode, Encode};
//use sp_runtime::traits::{Saturating, CheckedAdd, CheckedDiv, Zero};
//use rstd::ops::Rem;
//use sp_std::vec::Vec;

pub trait Trait: frame_system::Trait  + timestamp::Trait
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

// Logger target
const LOG: &str = "encointer";

decl_storage! {
    trait Store for Module<T: Trait> as Bazaar {
        // Maps the shop or article owner to the respective items 
        // TODO: Necessary?
        pub ShopsOwned get(fn shops_owned): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<Shop>;
        pub ArticlesOwned get(fn articles_owned): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<Article>; 
        // The set of all shops and articles.
        // TODO: Neccessary for Item to have an owner? To send messages to owner in case of questions to item?
        // Can content(URL) also be the key of the hash map?
        ShopRegistry get(fn shop_registry): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) ShopIdentifier => (T::AccountId, content);
        ArticleRegistry get(fn article_registry): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) ArticleIdentifier => (T::AccountId, content, ShopIdentifier);
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // initialisation
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Allow a user to create a shop.
        #[weight = 10_000]
        pub fn upload_shop(origin, cid: CurrencyIdentifier, shop: Shop, content: TYPE) {
            // Check that the extrinsic was signed and get the signer.
            let sender = ensure_signed(origin)?;

            let mut ownedShops = ShopsOwned::<T>::get(cid, &sender);

            // Verify that the specified shop has not already been created.
            ensure!(!ShopRegistry::<T>::contains_key(cid, &shop), Error::<T>::ShopAlreadyCreated);   
            // Get the index of the last entry of the Shop vector          
            match ownedShops.binary_search(cid, &shop) {
                // If the search succeeds, the shop was already created
                Ok(_) => Err(Error::<T>::ShopAlreadyCreated),
                // If the search fails, the shop can be inserted into the owned list
                Err(index) => {
                    ownedShops.insert(&index, shop.clone());
                    ShopsOwned::<T>::insert(cid, &sender, ownedShops);
                }
            }
            // Add the shop to the community registry.
            ShopRegistry::<T>::insert((cid, &shop), &sender, content);

            // Emit an event that the shop was created.
            Self::deposit_event(RawEvent::ShopCreated(sender, shop));
        }

        /// Allow a user to revoke their shop.
        #[weight = 10_000]
        pub fn revoke_claim(origin, cid:CurrencyIdentifier, shop: Shop) {
            // Check that the extrinsic was signed and get the signer.
            let sender = ensure_signed(origin)?;

            let mut ownedShops = ShopsOwned::<T>::get(cid, &sender);

            // Verify that the specified shop is existing.
            ensure!(ShopRegistry::<T>::contains_key(cid, &shop), Error::<T>::NoSuchShop);

            // Get the index of the shop in the owner list.
            match ownedShops.binary_search(cid, &shop) {
                // If the search succeeds, delete the respective entry.
                Ok(index) => {
                    ownedShops.remove(&index);
                    ShopsOwned::<T>::insert(cid, &sender, ownedShops);                    
                },
                // If the search fails, no such shop is owned.
                Err(_) => Err(Error::<T>::NoSuchShop),                
            }
            // Emit an event that the shop was removed.
            Self::deposit_event(RawEvent::ShopRemoved(sender, shop));
        }
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
        /// Event emitted when a shop is uploaded. [who, shop]
        ShopCreated(AccountId, Shop),
        /// Event emitted when a shop is revoked by the owner. [who, shop]
        ShopRemoved(AccountId, Shop),
    }
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// no such shop exisiting that could be deleted
        NoSuchShop,
        /// shop can not be created twice
        ShopAlreadyCreated,
	}
}

#[cfg(test)]
mod tests;
