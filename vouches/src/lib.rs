// Copyright (c) 2019 Encointer Association
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
use encointer_primitives::vouches::{Vouch, VouchKind, VouchQuality};
use frame_system::{self as frame_system, ensure_signed, pallet_prelude::OriginFor};
use log::info;
pub use pallet::*;
use sp_std::convert::TryInto;
pub use weights::WeightInfo;
// Logger target
const LOG: &str = "encointer::vouches";

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod weights;
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;
		#[pallet::constant]
		type MaxVouchesPerAttester: Get<u32>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::vouch_for(), DispatchClass::Normal, Pays::Yes))]
		pub fn vouch_for(
			origin: OriginFor<T>,
			attestee: T::AccountId,
			vouch_kind: VouchKind,
			quality: VouchQuality,
		) -> DispatchResultWithPostInfo {
			let attester = ensure_signed(origin)?;
			let now = <pallet_timestamp::Pallet<T>>::get();
			let vouch =
				Vouch { protected: false, timestamp: now, vouch_kind, quality: quality.clone() };
			<Vouches<T>>::try_mutate(
				&attestee,
				&attester,
				|vouches| -> DispatchResultWithPostInfo {
					vouches.try_push(vouch).map_err(|_| Error::<T>::TooManyVouchesForAttestee)?;
					Ok(().into())
				},
			)?;
			info!(target: LOG, "vouching: {:?} for {:?}, vouch type: {:?}, quality: {:?}", attester, attestee, vouch_kind, quality);
			Self::deposit_event(Event::VouchedFor { attestee, attester, vouch_kind });
			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// someone or something has vouched for someone or something
		VouchedFor { attestee: T::AccountId, attester: T::AccountId, vouch_kind: VouchKind },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The calling attester has too many vouches for this attestee
		TooManyVouchesForAttestee,
	}

	#[pallet::storage]
	#[pallet::getter(fn vouches)]
	pub(super) type Vouches<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<Vouch<T::Moment>, T::MaxVouchesPerAttester>,
		ValueQuery,
	>;
}
