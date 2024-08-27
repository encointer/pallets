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
use encointer_primitives::communities::CommunityIdentifier;
use frame_support::{
    traits::{Currency, ExistenceRequirement::KeepAlive, Get},
    PalletId,
};
use log::info;
use parity_scale_codec::Decode;
use sp_core::H256;
use sp_runtime::traits::Hash;

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

    #[pallet::pallet]
    pub struct Pallet<T>(PhantomData<T>);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: Currency<Self::AccountId>;

        /// The treasuries' pallet id, used for deriving sovereign account IDs per community.
        #[pallet::constant]
        type PalletId: Get<PalletId>;
    }

    impl<T: Config> Pallet<T>
    where
        sp_core::H256: From<<T as frame_system::Config>::Hash>,
        T::AccountId: AsRef<[u8; 32]>,
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

        /// returns the account id where remaining funds of closed faucets go
        pub fn do_spend_native(
            maybecid: Option<CommunityIdentifier>,
            beneficiary: T::AccountId,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let treasury = Self::get_community_treasury_account_unchecked(maybecid);
            T::Currency::transfer(&treasury, &beneficiary, amount, KeepAlive)?;
            info!(target: LOG, "treasury spent native: {:?}, {:?} to {:?}", maybecid, amount, beneficiary);
            Self::deposit_event(Event::SpentNative { treasury, beneficiary, amount });
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub (super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// treasury spent native tokens from community `cid` to `beneficiary` amounting `amount`
        SpentNative { treasury: T::AccountId, beneficiary: T::AccountId, amount: BalanceOf<T> },
    }
}
