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
use encointer_primitives::vouches::{Vouch, VouchQuality, VouchType};

use frame_system::{self as frame_system, ensure_signed, pallet_prelude::OriginFor};
use log::info;
pub use pallet::*;
use sp_core::H256;
use sp_std::convert::TryInto;
pub use weights::WeightInfo;

// Logger target
const LOG: &str = "encointer";

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
		type MaxQualitiesPerVouch: Get<u32>;
		#[pallet::constant]
		type MaxVouchesPerAttester: Get<u32>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::register_purpose(), DispatchClass::Normal, Pays::Yes))]
		pub fn vouch_for(
			origin: OriginFor<T>,
			attestee: T::AccountId,
			vouch_type: VouchType,
			qualities: BoundedVec<T::MaxQualitiesPerVouch, VouchQuality>,
		) -> DispatchResultWithPostInfo
		where
			<T as pallet::Config>::MaxQualitiesPerVouch: Clone,
		{
			let attester = ensure_signed(origin)?;
			let now = <pallet_timestamp::Pallet<T>>::get();
			Self::do_vouch_for(attester, attestee, now, vouch_type, qualities)?;
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn do_vouch_for(
			attester: T::AccountId,
			attestee: T::AccountId,
			timestamp: T::Moment,
			vouch_type: VouchType,
			qualities: BoundedVec<T::MaxQualitiesPerVouch, VouchQuality>,
		) -> Result<(), Error<T>> {
			let vouch = Vouch { protected: false, timestamp, vouch_type, qualities };
			<Vouches<T>>::try_mutate(&attestee, &attester, |vouches| {
				*vouches.push(vouch).map_err(|| Error::<T>::TooManyVouchesForAttestee);
				Ok(())
			})?;
			info!(target: LOG, "vouching: {:?} for {:?}, vouch type: {:?}, attached qualities: {:?}", attester, attestee, vouch_type, qualities.len());
			Self::deposit_event(Event::VouchedFor { attestee, attester, vouch_type });
			Ok(())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// someone or something has vouched for someone or something
		VouchedFor { attestee: T::AccountId, attester: T::AccountId, vouch_type: VouchType },
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
		Identity,
		T::AccountId,
		Identity,
		T::AccountId,
		BoundedVec<T::MaxVouchesPerAttester, Vouch<T::MaxQualitiesPerVouch, T::Moment>>,
		ValueQuery,
	>;
}
