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
	communities::CommunityIdentifier,
	democracy::{Proposal, ProposalAction, ProposalIdType, ReputationVec},
};
use frame_support::traits::Get;

// Logger target
//const LOG: &str = "encointer";

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
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + encointer_scheduler::Config + encointer_ceremonies::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		#[pallet::constant]
		type MaxReputationVecLength: Get<u32>;
		#[pallet::constant]
		type ConfirmationPeriod: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type ProposalLifetime: Get<BlockNumberFor<Self>>;
		#[pallet::constant]
		type MinTurnout: Get<u128>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProposalEnacted,
	}

	#[pallet::error]
	#[derive(PartialEq, Eq)]
	pub enum Error<T> {
		ProposalIdOutOfBounds,
		InexistentProposal,
		VoteCountOverflow,
		BoundedVecError,
		ProposalCannotBeUpdated,
	}

	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type Proposals<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalIdType, Proposal<BlockNumberFor<T>>, OptionQuery>;

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
	// TODO set default value
	#[pallet::storage]
	#[pallet::getter(fn cancelled_at_block)]
	pub(super) type CancelledAtBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalActionIdentifier, BlockNumberFor<T>, ValueQuery>;

	#[derive(frame_support::DefaultNoBound)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub proposal_count: ProposalIdType,
		#[serde(skip)]
		pub _config: sp_std::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			<ProposalCount<T>>::put(&self.proposal_count);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10000)]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal_action: ProposalAction,
		) -> DispatchResultWithPostInfo {
			let _sender = ensure_signed(origin)?;
			let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();
			let current_proposal_id = Self::proposal_count();
			let next_proposal_id = current_proposal_id
				.checked_add(1u128)
				.ok_or(Error::<T>::ProposalIdOutOfBounds)?;
			let current_block = frame_system::Pallet::<T>::block_number();
			let proposal = Proposal {
				start: current_block,
				start_cindex: cindex,
				state: ProposalState::Ongoing,
				action: proposal_action,
			};
			<Proposals<T>>::insert(next_proposal_id, proposal);
			<ProposalCount<T>>::put(next_proposal_id);
			<Tallies<T>>::insert(next_proposal_id, Tally { turnout: 0, ayes: 0 });
			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10000)]
		pub fn vote(
			origin: OriginFor<T>,
			proposal_id: ProposalIdType,
			vote: Vote,
			reputations: ReputationVecOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let tally = <Tallies<T>>::get(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			let maybe_cid = match Self::proposals(proposal_id)
				.ok_or(Error::<T>::InexistentProposal)?
				.action
				.get_access_policy()
			{
				ProposalAccessPolicy::Community(cid) => Some(cid),
				_ => None,
			};
			let valid_reputations =
				Self::valid_reputations(proposal_id, &sender, &reputations, maybe_cid)?;
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
		/// 3. originate in the correct community (for Community AccessPolicy)
		/// 4. are within proposal.start_cindex - reputation_lifetime and proposal.start_cindex - 2
		pub fn valid_reputations(
			proposal_id: ProposalIdType,
			account_id: &T::AccountId,
			reputations: &ReputationVecOf<T>,
			maybe_cid: Option<CommunityIdentifier>,
		) -> Result<ReputationVecOf<T>, Error<T>> {
			let reputation_lifetime = <encointer_ceremonies::Pallet<T>>::reputation_lifetime();
			let mut valid_reputations = Vec::<CommunityCeremony>::new();
			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;

			for community_ceremony in reputations {
				if community_ceremony.1 < proposal.start_cindex - reputation_lifetime ||
					community_ceremony.1 > proposal.start_cindex - 2
				{
					continue
				}

				if let Some(cid) = maybe_cid {
					if community_ceremony.0 != cid {
						continue
					}
				}
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

		/// Updates the proposal state
		/// If the state is changed to Approved, the proposal will be enacted
		/// In case of enactment, the function returns true
		pub fn update_proposal_state(proposal_id: ProposalIdType) -> Result<bool, Error<T>> {
			let mut proposal =
				Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			ensure!(proposal.state.can_update(), Error::<T>::ProposalCannotBeUpdated);
			let mut enacted = false;
			let current_block = frame_system::Pallet::<T>::block_number();
			let proposal_action_identifier = proposal.action.get_identifier();
			let cancelled_at_block = Self::cancelled_at_block(proposal_action_identifier);
			let proposal_cancelled = proposal.start < cancelled_at_block;
			let proposal_too_old = current_block - proposal.start > T::ProposalLifetime::get();
			if proposal_cancelled || proposal_too_old {
				proposal.state = ProposalState::Cancelled;
			} else {
				// passing
				if Self::is_passing(proposal_id)? {
					// confirming
					if let ProposalState::Confirming { since } = proposal.state {
						// confirmed longer than period
						if current_block - since > T::ConfirmationPeriod::get() {
							proposal.state = ProposalState::Approved;
							Self::enact_proposal(proposal_id)?;
							<CancelledAtBlock<T>>::insert(
								proposal_action_identifier,
								current_block,
							);
							enacted = true;
						}
					// not confirming
					} else {
						proposal.state = ProposalState::Confirming { since: current_block };
					}
				// not passing
				} else {
					// confirming
					if let ProposalState::Confirming { since: _ } = proposal.state {
						proposal.state = ProposalState::Ongoing;
					}
				}
			}
			<Proposals<T>>::insert(proposal_id, proposal);
			Ok(enacted)
		}

		pub fn is_passing(proposal_id: ProposalIdType) -> Result<bool, Error<T>> {
			let tally = Self::tallies(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			if tally.turnout < T::MinTurnout::get() {
				return Ok(false)
			}
			// TODO replace with AQB
			if tally.ayes < tally.turnout / 2 {
				return Ok(false)
			}
			Ok(true)
		}
		fn enact_proposal(_proposal_id: ProposalIdType) -> Result<(), Error<T>> {
			Self::deposit_event(Event::ProposalEnacted);
			todo!();
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
