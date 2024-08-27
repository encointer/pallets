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

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_primitives::{
	ceremonies::ReputationCountType,
	common::PalletString,
	democracy::{Proposal, ProposalAction, ProposalIdType, ReputationVec},
	fixed::{transcendental::sqrt, types::U64F64},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
};

use frame_support::dispatch::DispatchErrorWithPostInfo;

use frame_support::{
	sp_runtime::{
		traits::{CheckedAdd, CheckedDiv, CheckedSub},
		SaturatedConversion,
	},
	traits::Get,
};
use pallet_encointer_scheduler::OnCeremonyPhaseChange;

pub use weights::WeightInfo;

#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::string::ToString;
use frame_support::traits::Currency;
// Logger target
//const LOG: &str = "encointer";

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub use pallet::*;

type ReputationVecOf<T> = ReputationVec<<T as Config>::MaxReputationCount>;
pub type BalanceOf<T> = <<T as pallet_encointer_treasuries::Config>::Currency as Currency<
	<T as frame_system::Config>::AccountId,
>>::Balance;

use pallet_encointer_ceremonies::Pallet as CeremoniesPallet;
use pallet_encointer_communities::Pallet as CommunitiesPallet;
use pallet_encointer_treasuries::Pallet as TreasuriesPallet;

#[allow(clippy::unused_unit)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use encointer_primitives::{
		communities::CommunityIdentifier,
		democracy::{Tally, *},
		reputation_commitments::{DescriptorType, PurposeIdType},
	};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_encointer_scheduler::Config
		+ pallet_encointer_ceremonies::Config
		+ pallet_encointer_communities::Config
		+ pallet_encointer_reputation_commitments::Config
		+ pallet_encointer_treasuries::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		/// Maximum reputation count to be supplied in the extrinsics.
		#[pallet::constant]
		type MaxReputationCount: Get<u32>;

		/// The Period in which the proposal has to be in passing state before it is approved.
		#[pallet::constant]
		type ConfirmationPeriod: Get<Self::Moment>;

		/// The total lifetime of a proposal.
		///
		/// If the proposal isn't approved within its lifetime, it will be cancelled.
		///
		/// Note: In cycles this must be smaller than `ReputationLifetime`, otherwise the eligible
		/// electorate will be 0.
		#[pallet::constant]
		type ProposalLifetime: Get<Self::Moment>;

		/// Minimum turnout in permill for a proposal to be considered as passing and entering the
		/// `Confirming` state.
		#[pallet::constant]
		type MinTurnout: Get<u128>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		///  proposal enacted
		ProposalEnacted {
			proposal_id: ProposalIdType,
		},
		ProposalSubmitted {
			proposal_id: ProposalIdType,
			proposal_action: ProposalAction<T::AccountId, BalanceOf<T>>,
		},
		VotePlaced {
			proposal_id: ProposalIdType,
			vote: Vote,
			num_votes: u128,
		},
		VoteFailed {
			proposal_id: ProposalIdType,
			vote: Vote,
		},
		ProposalStateUpdated {
			proposal_id: ProposalIdType,
			proposal_state: ProposalState<T::Moment>,
		},
		EnactmentFailed {
			proposal_id: ProposalIdType,
			reason: DispatchErrorWithPostInfo,
		},
		PetitionApproved {
			cid: Option<CommunityIdentifier>,
			text: PalletString,
		},
	}

	#[pallet::error]
	#[derive(PartialEq, Eq)]
	pub enum Error<T> {
		/// proposal id out of bounds
		ProposalIdOutOfBounds,
		/// inexistent proposal
		InexistentProposal,
		/// vote count overflow
		VoteCountOverflow,
		/// bounded vec error
		BoundedVecError,
		/// proposal cannot be updated
		ProposalCannotBeUpdated,
		/// error when computing adaptive quorum biasing
		AQBError,
		/// cannot submit new proposal as a proposal of the same type is waiting for enactment
		ProposalWaitingForEnactment,
		/// reputation commitment purpose could not be created
		PurposeIdCreationFailed,
		/// error when doing math operations
		MathError,
	}

	/// Unique `PurposeIds` of a `Proposal`.
	///
	/// This is used to prevent reuse of a reputation for the same `PurposeId`.
	#[pallet::storage]
	#[pallet::getter(fn purpose_ids)]
	pub(super) type PurposeIds<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalIdType, PurposeIdType, OptionQuery>;

	/// All proposals that have ever been proposed including the past ones.
	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type Proposals<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ProposalIdType,
		Proposal<T::Moment, T::AccountId, BalanceOf<T>>,
		OptionQuery,
	>;

	/// Proposal count of all proposals to date.
	#[pallet::storage]
	#[pallet::getter(fn proposal_count)]
	pub(super) type ProposalCount<T: Config> = StorageValue<_, ProposalIdType, ValueQuery>;

	/// Tallies for the proposal corresponding to `ProposalId`.
	#[pallet::storage]
	#[pallet::getter(fn tallies)]
	pub(super) type Tallies<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalIdType, Tally, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_approved_proposal_for_action)]
	pub(super) type LastApprovedProposalForAction<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		ProposalActionIdentifier,
		(T::Moment, ProposalIdType),
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn enactment_queue)]
	pub(super) type EnactmentQueue<T: Config> =
		StorageMap<_, Blake2_128Concat, ProposalActionIdentifier, ProposalIdType, OptionQuery>;

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
			<ProposalCount<T>>::put(self.proposal_count);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
		T::AccountId: AsRef<[u8; 32]>,
	{
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::submit_proposal(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal_action: ProposalAction<T::AccountId, BalanceOf<T>>,
		) -> DispatchResultWithPostInfo {
			if Self::enactment_queue(proposal_action.clone().get_identifier()).is_some() {
				return Err(Error::<T>::ProposalWaitingForEnactment.into());
			}
			let _sender = ensure_signed(origin)?;
			let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
			let current_proposal_id = Self::proposal_count();
			let next_proposal_id = current_proposal_id
				.checked_add(1u128)
				.ok_or(Error::<T>::ProposalIdOutOfBounds)?;
			let now = <pallet_timestamp::Pallet<T>>::get();
			let proposal = Proposal {
				start: now,
				start_cindex: cindex,
				state: ProposalState::Ongoing,
				action: proposal_action.clone(),
				electorate_size: Self::get_electorate(cindex, proposal_action.clone())?,
			};

			let proposal_identifier =
				["democracyProposal".as_bytes(), next_proposal_id.to_string().as_bytes()].concat();

			let purpose_id =
				<pallet_encointer_reputation_commitments::Pallet<T>>::do_register_purpose(
					DescriptorType::try_from(proposal_identifier)
						.map_err(|_| <Error<T>>::PurposeIdCreationFailed)?,
				)?;

			<Proposals<T>>::insert(next_proposal_id, proposal);
			<PurposeIds<T>>::insert(next_proposal_id, purpose_id);
			<ProposalCount<T>>::put(next_proposal_id);
			<Tallies<T>>::insert(next_proposal_id, Tally { turnout: 0, ayes: 0 });
			Self::deposit_event(Event::ProposalSubmitted {
				proposal_id: next_proposal_id,
				proposal_action,
			});
			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight((<T as Config>::WeightInfo::vote(), DispatchClass::Normal, Pays::Yes))]
		pub fn vote(
			origin: OriginFor<T>,
			proposal_id: ProposalIdType,
			vote: Vote,
			reputations: ReputationVecOf<T>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let tally = <Tallies<T>>::get(proposal_id).ok_or(Error::<T>::InexistentProposal)?;

			// make sure we don't vote on proposal that can't update anymore
			Self::do_update_proposal_state(proposal_id)?;

			let num_votes =
				Self::validate_and_commit_reputations(proposal_id, &sender, &reputations)?;

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

			Self::do_update_proposal_state(proposal_id)?;

			if num_votes > 0 {
				Self::deposit_event(Event::VotePlaced { proposal_id, vote, num_votes })
			} else {
				Self::deposit_event(Event::VoteFailed { proposal_id, vote })
			}
			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::update_proposal_state(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn update_proposal_state(
			origin: OriginFor<T>,
			proposal_id: ProposalIdType,
		) -> DispatchResultWithPostInfo {
			let _sender = ensure_signed(origin)?;
			Self::do_update_proposal_state(proposal_id)?;
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T>
	where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
		T::AccountId: AsRef<[u8; 32]>,
	{
		/// Returns the cindexes eligible for voting on a proposal with `proposal_start`.
		///
		/// It is essentially the range of:
		/// 	`[proposal_start - reputation_lifetime + proposal_lifetime, proposal_start - 2]`
		///
		/// These boundaries ensure that we have a constant electorate to determine the
		/// approval threshold.
		/// * The lower bound ensures that the oldest reputation still exist at the end of the
		/// 	proposal lifetime.
		/// *	The upper bound ensures that the still dynamic reputation count of the
		/// 	cindex at submission time is not included.
		fn voting_cindexes(
			proposal_start: CeremonyIndexType,
		) -> Result<Vec<CeremonyIndexType>, Error<T>> {
			let proposal_lifetime_cycles: u32 = Self::proposal_lifetime_cycles()?.saturated_into();

			let voting_cindex_lower_bound = proposal_start
				.saturating_sub(CeremoniesPallet::<T>::reputation_lifetime())
				.saturating_add(proposal_lifetime_cycles);

			let cindexes = voting_cindex_lower_bound..=proposal_start.saturating_sub(2u32);

			Ok(cindexes.collect())
		}

		fn proposal_lifetime_cycles() -> Result<T::Moment, Error<T>> {
			let cycle_duration = <pallet_encointer_scheduler::Pallet<T>>::get_cycle_duration();

			// integer operation for ceil(proposal_lifetime / cycle_duration)
			T::ProposalLifetime::get()
				.checked_add(&cycle_duration)
				.and_then(|r| r.checked_sub(&T::Moment::saturated_from(1u64)))
				.and_then(|r| r.checked_div(&cycle_duration))
				.ok_or(Error::<T>::MathError)
		}

		/// Validates the reputations based on the following criteria and commits the reputations.
		/// Returns count of valid reputations.
		/// 1. are valid
		/// 2. have not been used to vote for proposal_id
		/// 3. originate in the correct community (for Community AccessPolicy)
		/// 4. are within proposal.start_cindex - reputation_lifetime + proposal_lifetime and
		///    proposal.start_cindex - 2
		pub fn validate_and_commit_reputations(
			proposal_id: ProposalIdType,
			account_id: &T::AccountId,
			reputations: &ReputationVecOf<T>,
		) -> Result<u128, Error<T>> {
			let mut eligible_reputation_count = 0u128;
			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			let maybe_cid = match proposal.action.get_access_policy() {
				ProposalAccessPolicy::Community(cid) => Some(cid),
				_ => None,
			};

			let purpose_id =
				Self::purpose_ids(proposal_id).ok_or(Error::<T>::InexistentProposal)?;

			for community_ceremony in reputations {
				if !Self::voting_cindexes(proposal.start_cindex)?.contains(&community_ceremony.1) {
					continue;
				}

				if let Some(cid) = maybe_cid {
					if community_ceremony.0 != cid {
						continue;
					}
				}

				if <pallet_encointer_reputation_commitments::Pallet<T>>::do_commit_reputation(
					account_id,
					community_ceremony.0,
					community_ceremony.1,
					purpose_id,
					None,
				)
				.is_err()
				{
					continue;
				}

				eligible_reputation_count += 1;
			}
			Ok(eligible_reputation_count)
		}

		/// Updates the proposal state.
		///
		/// If the state is changed to Approved, the proposal will be enacted.
		/// In case of enactment, the function returns true.
		pub fn do_update_proposal_state(proposal_id: ProposalIdType) -> Result<bool, Error<T>> {
			let mut proposal =
				Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			ensure!(proposal.state.can_update(), Error::<T>::ProposalCannotBeUpdated);
			let mut approved = false;
			let old_proposal_state = proposal.state;
			let now = <pallet_timestamp::Pallet<T>>::get();
			let proposal_action_identifier = proposal.action.clone().get_identifier();
			let last_approved_proposal_for_action =
				Self::last_approved_proposal_for_action(proposal_action_identifier);
			let proposal_cancelled_by_other = proposal.action.supersedes_same_action() &&
				last_approved_proposal_for_action.is_some() &&
				proposal.start < last_approved_proposal_for_action.unwrap().0;
			let proposal_too_old = now - proposal.start > T::ProposalLifetime::get();
			if proposal_cancelled_by_other {
				proposal.state =
					ProposalState::SupersededBy { id: last_approved_proposal_for_action.unwrap().1 }
			} else {
				// passing
				if Self::is_passing(proposal_id)? {
					// confirming
					if let ProposalState::Confirming { since } = proposal.state {
						// confirmed longer than period
						if now.checked_sub(&since).unwrap_or_default() >
							T::ConfirmationPeriod::get()
						{
							proposal.state = ProposalState::Approved;
							<EnactmentQueue<T>>::insert(proposal_action_identifier, proposal_id);
							<LastApprovedProposalForAction<T>>::insert(
								proposal_action_identifier,
								(now, proposal_id),
							);
							approved = true;
						}
					// not yet confirming
					} else if proposal_too_old {
						proposal.state = ProposalState::Rejected;
					} else {
						proposal.state = ProposalState::Confirming { since: now };
					}

				// not passing
				} else if proposal_too_old {
					proposal.state = ProposalState::Rejected;
				} else if let ProposalState::Confirming { since: _ } = proposal.state {
					proposal.state = ProposalState::Ongoing;
				}
			}
			<Proposals<T>>::insert(proposal_id, &proposal);
			if old_proposal_state != proposal.state {
				Self::deposit_event(Event::ProposalStateUpdated {
					proposal_id,
					proposal_state: proposal.state,
				});
			}
			Ok(approved)
		}

		pub fn get_electorate(
			start_cindex: CeremonyIndexType,
			proposal_action: ProposalAction<T::AccountId, BalanceOf<T>>,
		) -> Result<ReputationCountType, Error<T>> {
			let voting_cindexes = Self::voting_cindexes(start_cindex)?;

			let electorate = match proposal_action.get_access_policy() {
				ProposalAccessPolicy::Community(cid) =>
					Self::community_electorate(cid, voting_cindexes),
				ProposalAccessPolicy::Global => Self::global_electorate(voting_cindexes),
			};

			Ok(electorate)
		}

		fn community_electorate(
			cid: CommunityIdentifier,
			cindexes: Vec<CeremonyIndexType>,
		) -> ReputationCountType {
			cindexes
				.iter()
				.map(|cindex| CeremoniesPallet::<T>::reputation_count((cid, cindex)))
				.sum()
		}

		fn global_electorate(cindexes: Vec<CeremonyIndexType>) -> ReputationCountType {
			cindexes
				.iter()
				.map(|cindex| CeremoniesPallet::<T>::global_reputation_count(cindex))
				.sum()
		}

		fn positive_turnout_bias(e: u128, t: u128, a: u128) -> Option<bool> {
			// electorate e
			// turnout t
			// approval a

			// let nays n = t - a
			// approved if n / sqrt(t) < a / sqrt(e)
			// <==>
			// a > sqrt(e) * sqrt(t) / (sqrt(e) / sqrt(t) + 1)

			let sqrt_e = sqrt::<U64F64, U64F64>(U64F64::from_num(e)).ok()?;
			let sqrt_t = sqrt::<U64F64, U64F64>(U64F64::from_num(t)).ok()?;

			let approval_threshold = sqrt_e.checked_mul(sqrt_t).and_then(|r| {
				r.checked_div(sqrt_e.checked_div(sqrt_t).and_then(|r| r.checked_add(1u32.into()))?)
			})?;

			let approved = U64F64::from_num(a) > approval_threshold;

			Some(approved)
		}

		pub fn is_passing(proposal_id: ProposalIdType) -> Result<bool, Error<T>> {
			let tally = Self::tallies(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			let proposal = Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;
			let electorate = proposal.electorate_size;

			let turnout_permill = (tally.turnout * 1000).checked_div(electorate).unwrap_or(0);
			if turnout_permill < T::MinTurnout::get() {
				return Ok(false);
			}

			Self::positive_turnout_bias(electorate, tally.turnout, tally.ayes)
				.ok_or(Error::<T>::AQBError)
		}
		pub fn enact_proposal(proposal_id: ProposalIdType) -> DispatchResultWithPostInfo {
			let mut proposal =
				Self::proposals(proposal_id).ok_or(Error::<T>::InexistentProposal)?;

			match proposal.action {
				ProposalAction::AddLocation(cid, location) => {
					CommunitiesPallet::<T>::do_add_location(cid, location)?;
				},
				ProposalAction::RemoveLocation(cid, location) => {
					CommunitiesPallet::<T>::do_remove_location(cid, location)?;
				},
				ProposalAction::UpdateCommunityMetadata(cid, ref community_metadata) => {
					CommunitiesPallet::<T>::do_update_community_metadata(
						cid,
						community_metadata.clone(),
					)?;
				},
				ProposalAction::UpdateDemurrage(cid, demurrage) => {
					CommunitiesPallet::<T>::do_update_demurrage(cid, demurrage)?;
				},
				ProposalAction::UpdateNominalIncome(cid, nominal_income) => {
					CommunitiesPallet::<T>::do_update_nominal_income(cid, nominal_income)?;
				},
				ProposalAction::SetInactivityTimeout(inactivity_timeout) => {
					CeremoniesPallet::<T>::do_set_inactivity_timeout(inactivity_timeout)?;
				},
				ProposalAction::Petition(maybe_cid, ref petition) => {
					Self::deposit_event(Event::PetitionApproved {
						cid: maybe_cid,
						text: petition.clone(),
					});
				},
				ProposalAction::SpendNative(maybe_cid, ref beneficiary, amount) => {
					TreasuriesPallet::<T>::do_spend_native(maybe_cid, beneficiary.clone(), amount)?;
				},
			};

			proposal.state = ProposalState::Enacted;
			<Proposals<T>>::insert(proposal_id, proposal);
			Self::deposit_event(Event::ProposalEnacted { proposal_id });
			Ok(().into())
		}
	}
}

impl<T: Config> OnCeremonyPhaseChange for Pallet<T>
where
	sp_core::H256: From<<T as frame_system::Config>::Hash>,
	T::AccountId: AsRef<[u8; 32]>,
{
	fn on_ceremony_phase_change(new_phase: CeremonyPhaseType) {
		match new_phase {
			CeremonyPhaseType::Assigning => {
				// safe as EnactmentQueue has one key per ProposalActionType and those are bounded
				<EnactmentQueue<T>>::iter().for_each(|p| {
					if let Err(e) = Self::enact_proposal(p.1) {
						Self::deposit_event(Event::EnactmentFailed { proposal_id: p.1, reason: e })
					}
				});
				// remove all keys from the map
				<EnactmentQueue<T>>::translate::<ProposalIdType, _>(|_, _| None);
			},
			CeremonyPhaseType::Attesting => {},
			CeremonyPhaseType::Registering => {},
		}
	}
}

mod migrations;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
mod weights;
//
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
