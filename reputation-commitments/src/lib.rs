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
use encointer_primitives::{
	communities::CommunityIdentifier,
	reputation_commitments::{DescriptorType, PurposeIdType},
	scheduler::CeremonyIndexType,
};
use frame_system::{self as frame_system, ensure_signed, pallet_prelude::OriginFor};
pub use pallet::*;
use sp_core::H256;
use sp_std::convert::TryInto;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config + encointer_ceremonies::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn register_purpose(
			origin: OriginFor<T>,
			descriptor: DescriptorType,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;
			Self::do_register_purpose(descriptor)?;
			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn commit_reputation(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
			purpose: PurposeIdType,
			commitment_hash: Option<H256>,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			Self::do_commit_reputation(&from, cid, cindex, purpose, commitment_hash)?;
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn do_register_purpose(descriptor: DescriptorType) -> Result<PurposeIdType, Error<T>> {
			let current_id = Self::current_purpose_id();
			let next_id = current_id.checked_add(1).ok_or(<Error<T>>::PurposeRegistryOverflow)?;

			<CurrentPurposeId<T>>::put(next_id);
			<Purposes<T>>::insert(current_id, descriptor.clone());
			Self::deposit_event(Event::RegisteredCommitmentPurpose(current_id, descriptor));
			Ok(current_id)
		}

		pub fn do_commit_reputation(
			account: &T::AccountId,
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
			purpose: PurposeIdType,
			commitment_hash: Option<H256>,
		) -> Result<(), Error<T>> {
			if !<Purposes<T>>::contains_key(purpose) {
				return Err(<Error<T>>::InexistentPurpose)
			}

			if !<encointer_ceremonies::Pallet<T>>::participant_reputation((cid, cindex), account)
				.is_verified()
			{
				return Err(<Error<T>>::NoReputation)
			}

			if <Commitments<T>>::contains_key(purpose, (&account, cid, cindex)) {
				return Err(<Error<T>>::AlreadyCommited)
			}

			<Commitments<T>>::insert(purpose, (&account, cid, cindex), commitment_hash);
			Self::deposit_event(Event::CommitedReputation(
				account.clone(),
				cid,
				cindex,
				purpose,
				commitment_hash,
			));
			Ok(())
		}
	}

	#[pallet::genesis_config]
	#[derive(Default)]
	pub struct GenesisConfig {}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// commitment purpose registered
		RegisteredCommitmentPurpose(PurposeIdType, DescriptorType),
		/// reputation commited for purpose
		CommitedReputation(
			T::AccountId,
			CommunityIdentifier,
			CeremonyIndexType,
			PurposeIdType,
			Option<H256>,
		),
	}

	#[pallet::error]
	#[derive(PartialEq)]
	pub enum Error<T> {
		/// Participant already commited their reputation for this purpose
		AlreadyCommited,
		/// Participant does not have reputation for the specified cid, cindex
		NoReputation,
		/// Purposose registry is full
		PurposeRegistryOverflow,
		/// Inexsitent purpose
		InexistentPurpose,
	}

	#[pallet::storage]
	#[pallet::getter(fn current_purpose_id)]
	pub(super) type CurrentPurposeId<T: Config> = StorageValue<_, PurposeIdType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn purposes)]
	pub(super) type Purposes<T: Config> =
		StorageMap<_, Identity, PurposeIdType, DescriptorType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn commitments)]
	pub(super) type Commitments<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		PurposeIdType,
		Identity,
		(T::AccountId, CommunityIdentifier, CeremonyIndexType),
		Option<H256>,
		ValueQuery,
	>;
}
