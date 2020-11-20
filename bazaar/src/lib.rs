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
#[macro_use]
extern crate approx;

use frame_support::{
    decl_event, decl_module, decl_storage, decl_error,
    dispatch::DispatchResult,
    ensure,
    storage::{StorageDoubleMap, StorageMap},
    traits::Get,
    weights::{DispatchClass, Pays},
    debug
};
use rstd::prelude::*;
use sp_core::RuntimeDebug;
use frame_system::ensure_signed;
use sp_timestamp::OnTimestampSet;

use rstd::{cmp::min, convert::TryInto};
use codec::{Decode, Encode};
use sp_runtime::traits::{Saturating, CheckedAdd, CheckedDiv, Zero, IdentifyAccount, Member, Verify, CheckedSub};
use rstd::ops::Rem;

use encointer_currencies::{CurrencyIdentifier, Location, Degree, LossyInto};
use encointer_balances::BalanceType;
use encointer_scheduler::{CeremonyIndexType, CeremonyPhaseType, OnCeremonyPhaseChange};

pub trait Trait: frame_system::Trait 
    + timestamp::Trait
    + encointer_currencies::Trait 
    + encointer_balances::Trait 
    + encointer_scheduler::Trait
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
    type Public: IdentifyAccount<AccountId = Self::AccountId>;
    type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode;
}
// Logger target
const LOG: &str = "encointer";

pub type ShopIdentifier = u64; //URL
pub type ArticleIdentifier = u64; //URL

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfAttendance<Signature, AccountId> {
    pub prover_public: AccountId,
    pub ceremony_index: CeremonyIndexType,
    pub currency_identifier: CurrencyIdentifier,
    pub attendee_public: AccountId,
    pub attendee_signature: Signature,
}

decl_storage! {
    trait Store for Module<T: Trait> as Bazaar {
        // Maps the shop or article owner to the respective items 
        pub ShopsOwned get(fn shops_owned): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<ShopIdentifier>;
        pub ArticlesOwned get(fn articles_owned): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) T::AccountId => Vec<ArticleIdentifier>; 
        // The set of all shops and articles
        ShopRegistry get(fn shop_registry): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) ShopIdentifier => T::AccountId;
        ArticleRegistry get(fn article_registry): double_map hasher(blake2_128_concat) CurrencyIdentifier, hasher(blake2_128_concat) ArticleIdentifier => (T::AccountId, ShopIdentifier);
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
        /// Event emitted when a shop is uploaded. [who, shop]
        ShopCreated(AccountId, ShopIdentifier),
        /// Event emitted when a shop is revoked by the owner. [who, shop]
        ShopRemoved(AccountId, ShopIdentifier),
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

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // initialisation
      //  type Error = Error<T>;
        fn deposit_event() = default;

        /// Allow a user to create a shop
        #[weight = 10_000]
        pub fn upload_shop(origin, cid: CurrencyIdentifier, shop: ShopIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer
            let sender = ensure_signed(origin)?;

            let mut owned_shops = ShopsOwned::<T>::get(cid, &sender);

            // Verify that the specified shop has not already been created
            ensure!(!ShopRegistry::<T>::contains_key(cid, &shop), Error::<T>::ShopAlreadyCreated);   
            // Get the index of the last entry of the Shop vector          
            match owned_shops.binary_search(&shop) {
                // If the search succeeds, the shop was already created
                Ok(_) => Err(<Error<T>>::ShopAlreadyCreated.into()),

                // If the search fails, the shop can be inserted into the owned list
                Err(index) => {
                    // Add the shop to the owned registry
                    owned_shops.insert(index, shop.clone());
                    ShopsOwned::<T>::insert(cid, &sender, owned_shops);
                    // Add the shop to the community registry
                    ShopRegistry::<T>::insert(cid, &shop, &sender);  
                    // Emit an event that the shop was created
                    Self::deposit_event(RawEvent::ShopCreated(sender, shop));
                    Ok(())
                },
            }            
        }

        /// Allow a user to revoke their shop.
        #[weight = 10_000]
        pub fn revoke_claim(origin, cid:CurrencyIdentifier, shop: ShopIdentifier) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            let sender = ensure_signed(origin)?;

            let mut owned_shops = ShopsOwned::<T>::get(cid, &sender);

            // Verify that the specified shop is existing.
            ensure!(ShopRegistry::<T>::contains_key(cid, &shop), Error::<T>::NoSuchShop);

            // Get the index of the shop in the owner list.
            match owned_shops.binary_search(&shop) {
                // If the search succeeds, delete the respective entry.
                Ok(index) => {
                    // Remove the shop from the owned registry
                    owned_shops.remove(index);
                    ShopsOwned::<T>::insert(cid, &sender, owned_shops);
                    // Remove the shop from the community registry.
                    ShopRegistry::<T>::remove(cid, &shop);
                    // Emit an event that the shop was removed.
                    Self::deposit_event(RawEvent::ShopRemoved(sender, shop));    
                    Ok(())             
                },
                // If the search fails, no such shop is owned.
                Err(_) => Err(Error::<T>::NoSuchShop.into()),       
            }                   
        }
    }
}

#[cfg(test)]
mod tests;