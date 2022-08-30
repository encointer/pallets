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

//! # Encointer Democracy Module
//!

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_primitives::{
	ceremonies::CommunityCeremony,
	democracy::{Proposal, ProposalIdType, ReputationVec},
};
use frame_support::traits::Get;

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;

type ReputationVecOf<T> = ReputationVec<<T as pallet::Config>::MaxReputationVecLength>;
#[allow(clippy::unused_unit)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use encointer_primitives::democracy::{Tally, *};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + encointer_scheduler::Config + encointer_ceremonies::Config
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		#[pallet::constant]
		type MaxReputationVecLength: Get<u32>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SomeEvent,
	}

	#[pallet::error]
	pub enum Error<T> {
		ProposalIdOutOfBounds,
		InexistentProposal,
		VoteCountOverflow,
		BoundedVecError,
	}

	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type Proposals<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalIdType, Proposal<T::BlockNumber>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub(super) type ProposalCount<T: Config> = StorageValue<_, ProposalIdType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn tallies)]
	pub(super) type Tallies<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalIdType, Tally, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn vote_entries)]
	pub(super) type VoteEntries<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		ProposalIdType,
		Blake2_128Concat,
		VoteEntry<T::AccountId>,
		(),
		ValueQuery,
	>;

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub proposal_count: ProposalIdType,
	}

	#[cfg(feature = "std")]
	#[allow(clippy::derivable_impls)]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self { proposal_count: 0u128 }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			<ProposalCount<T>>::put(&self.proposal_count);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal: Proposal<T::BlockNumber>,
		) -> DispatchResultWithPostInfo {
			let _sender = ensure_signed(origin)?;
			let current_proposal_id = Self::proposal_count();
			let next_proposal_id = current_proposal_id
				.checked_add(1u128)
				.ok_or(Error::<T>::ProposalIdOutOfBounds)?;
			<Proposals<T>>::insert(next_proposal_id, proposal);
			<ProposalCount<T>>::put(next_proposal_id);
			<Tallies<T>>::insert(next_proposal_id, Tally { turnout: 0, ayes: 0 });
			Ok(().into())
		}

		#[pallet::weight(10000)]
		pub fn vote(
			origin: OriginFor<T>,
			proposal_id: ProposalIdType,
			vote: Vote,
			reputations: ReputationVecOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let tally = <Tallies<T>>::get(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			let valid_reputations = Self::valid_reputations(proposal_id, &sender, &reputations)?;
			let num_votes = valid_reputations.len() as u128;

			let ayes = match vote {
				Vote::Aye => num_votes,
				Vote::Nay => 0,
			};

			let new_tally = Tally {
				turnout: tally
					.turnout
					.checked_add(num_votes)
					.ok_or(Error::<T>::VoteCountOverflow)?,
				ayes: tally.ayes.checked_add(ayes).ok_or(Error::<T>::VoteCountOverflow)?,
			};

			<Tallies<T>>::insert(proposal_id, new_tally);
			for community_ceremony in valid_reputations {
				<VoteEntries<T>>::insert(proposal_id, (&sender, community_ceremony), ());
			}

			Ok(().into())
		}
	}
	impl<T: Config> Pallet<T> {
		/// Returns the reputations that
		/// 1. are valid
		/// 2. have not been used to vote for proposal_id
		pub fn valid_reputations(
			proposal_id: ProposalIdType,
			account_id: &T::AccountId,
			reputations: &ReputationVecOf<T>,
		) -> Result<ReputationVecOf<T>, Error<T>> {
			let mut valid_reputations = Vec::<CommunityCeremony>::new();
			for community_ceremony in reputations {
				if <VoteEntries<T>>::contains_key(proposal_id, (account_id, community_ceremony)) {
					continue
				}
				if <encointer_ceremonies::Pallet<T>>::validate_reputation(
					account_id,
					&community_ceremony.0,
					community_ceremony.1,
				) {
					valid_reputations.push(*community_ceremony);
				}
			}
			BoundedVec::try_from(valid_reputations).map_err(|_e| Error::<T>::BoundedVecError)
		}
	}
}

// mod weights;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
//
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
