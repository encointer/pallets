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
//! - creating new bazaar entries
//! - removing existing entries
//!

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage,
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
        ShopsOwned get(fn shops_owned): map hasher(blake2_128_concat) T::AccountId => Vec<Shop>; 
        ArticlesOwned get(fn articles_owned): map hasher(blake2_128_concat) T::AccountId => Vec<Article>; 
        // The set of all shops and articles.
        // TODO: Neccessary for Item to have an owner? To send messages to owner in case of questions to item?
        // Can content also be the key of the hash map?
        Shops get(fn shops): map hasher(blake2_128_concat) Shop => (T::AccountId, content);
        Articles get(fn articles): map hasher(blake2_128_concat) Article => (T::AccountId, content);
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // initialisation
        type Error = Error<T>;
        fn deposit_event() = default;

        /// Allow a user to create a shop.
        #[weight = 10_000]
        fn upload_shop(origin, shop: Shop, content: TYPE) {
            // Check that the extrinsic was signed and get the signer.
            let sender = ensure_signed(origin)?;

            // Verify that the specified shop has not already been created.
            ensure!(!Shops::<T>::contains_key(&shop), Error::<T>::ShopAlreadyCreated);            

            // Create the shop and add it to the shop list of the creator.
            // TODO: necessary two way? Or better solution possible?
            Shops::<T>::insert(&shop, (&sender, content));

            match members.binary_search(&new_member)
            // Assumption: In case shop is not in Shops storage, it is also not in ShopsOwned of the sender.

            //ShopsOwned::<T>::insert(&sender, (&shop));

            

            // Emit an event that the shop was created.
            Self::deposit_event(RawEvent::ShopCreated(sender, shop));
        }

        /// Allow the owner to revoke their shop.
        #[weight = 10_000]
        fn revoke_claim(origin, shop: Vec<Shop>) {
            // Check that the extrinsic was signed and get the signer.
            let sender = ensure_signed(origin)?;

            // Verify that the specified shop has been uploaded.
            ensure!(Shops::<T>::contains_key(&shop), Error::<T>::NoSuchShop);

            // Get owner of the shop.
            //let (owner, _) = Proofs::<T>::get(&proof);
            

            // Verify that sender of the current call is the claim owner.
            ensure!(sender == owner, Error::<T>::NotProofOwner);

            // Remove claim from storage.
            Proofs::<T>::remove(&proof);

            // Emit an event that the claim was erased.
            Self::deposit_event(RawEvent::ClaimRevoked(sender, proof));
        }
    }
}

decl_event! {
    pub enum Event<T> where AccountId = <T as frame_system::Trait>::AccountId {
        /// Event emitted when a shop is uploaded. [who, shop]
        ShopCreated(AccountId, Shop),
        /// Event emitted when a shop is revoked by the owner. [who, shop]
        ShopRevoked(AccountId, Shop),
    }
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		/// minimum distance violated towards pole
        MinimumDistanceViolationToPole,
        /// minimum distance violated towards dateline
        MinimumDistanceViolationToDateLine,
        /// minimum distance violated towards other currency's location
		MinimumDistanceViolationToOtherCurrency,
	}
}



#[cfg(test)]
mod tests;
