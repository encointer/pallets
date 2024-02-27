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

//! # Encointer Ceremonies Module
//!
//! The Encointer Ceremonies module provides functionality for
//! - registering for upcoming ceremony
//! - meetup assignment
//! - attestation registry
//! - issuance of basic income
//!

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_ceremonies_assignment::{
	assignment_fn_inverse, generate_assignment_function_params, get_meetup_location_index,
	math::{checked_ceil_division, find_prime_below, find_random_coprime_below},
	meetup_index, meetup_location, meetup_time,
};
use encointer_meetup_validation::*;
use encointer_primitives::{
	balances::BalanceType,
	ceremonies::*,
	communities::{CommunityIdentifier, Location, NominalIncome},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
	RandomNumberGenerator,
};
use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo, Pays},
	ensure,
	traits::{Get, Randomness},
	BoundedVec,
};
use frame_system::ensure_signed;
use log::{debug, error, info, trace, warn};
use pallet_encointer_scheduler::OnCeremonyPhaseChange;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::bounded::BoundedSlice;
use sp_runtime::traits::{IdentifyAccount, Member, Verify};
use sp_std::{
	cmp::{max, min},
	prelude::*,
	vec,
};
// Logger target
const LOG: &str = "encointer";

pub use pallet::*;
pub use weights::WeightInfo;

mod storage_helper;
#[allow(clippy::unused_unit)]
#[frame_support::pallet]
pub mod pallet {
	use super::{Pays, *};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);
	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_timestamp::Config
		+ pallet_encointer_communities::Config
		+ pallet_encointer_balances::Config
		+ pallet_encointer_scheduler::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type CeremonyMaster: EnsureOrigin<Self::RuntimeOrigin>;

		type Public: IdentifyAccount<AccountId = Self::AccountId>;
		type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;
		type RandomnessSource: Randomness<Self::Hash, BlockNumberFor<Self>>;
		// Target number of participants per meetup
		#[pallet::constant]
		type MeetupSizeTarget: Get<u64>;
		// Minimum meetup size
		#[pallet::constant]
		type MeetupMinSize: Get<u64>;
		// Divisor used to determine the ratio of newbies allowed in relation to other participants
		#[pallet::constant]
		type MeetupNewbieLimitDivider: Get<u64>;

		#[pallet::constant]
		type MaxAttestations: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::register_participant(), DispatchClass::Normal, Pays::Yes))]
		pub fn register_participant(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			proof: Option<ProofOfAttendance<T::Signature, T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let current_phase = <pallet_encointer_scheduler::Pallet<T>>::current_phase();
			ensure!(
				CeremonyPhaseType::is_registering_or_attesting(&current_phase),
				Error::<T>::RegisteringOrAttestationPhaseRequired
			);

			ensure!(
				<pallet_encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let mut cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();

			if current_phase == CeremonyPhaseType::Attesting {
				cindex += 1
			};

			if Self::is_registered(cid, cindex, &sender) {
				return Err(<Error<T>>::ParticipantAlreadyRegistered.into());
			}

			if let Some(p) = &proof {
				// we accept proofs from other communities as well. no need to ensure cid
				ensure!(sender == p.prover_public, Error::<T>::WrongProofSubject);
				ensure!(p.ceremony_index < cindex, Error::<T>::ProofAcausal);
				ensure!(
					p.ceremony_index >= cindex.saturating_sub(Self::reputation_lifetime()),
					Error::<T>::ProofOutdated
				);
				ensure!(
					Self::participant_reputation(
						(p.community_identifier, p.ceremony_index),
						&p.attendee_public
					)
					.is_verified_and_unlinked_for_cindex(cindex),
					Error::<T>::AttendanceUnverifiedOrAlreadyUsed
				);

				ensure!(p.verify_signature(), Error::<T>::BadProofOfAttendanceSignature);

				// this reputation must now be flagged so it can not be used again in the same cycle
				<ParticipantReputation<T>>::insert(
					(p.community_identifier, p.ceremony_index),
					&p.attendee_public,
					Reputation::VerifiedLinked(cindex),
				);
				// register participant as reputable
				<ParticipantReputation<T>>::insert(
					(cid, cindex),
					&sender,
					Reputation::UnverifiedReputable,
				);
			};

			let participant_type = Self::register(cid, cindex, &sender, proof.is_some())?;

			// invalidate reputation cache
			sp_io::offchain_index::set(&reputation_cache_dirty_key(&sender), &true.encode());

			debug!(target: LOG, "registered participant: {:?} as {:?}", sender, participant_type);
			Self::deposit_event(Event::ParticipantRegistered(cid, participant_type, sender));

			Ok(().into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight((<T as Config>::WeightInfo::upgrade_registration(), DispatchClass::Normal, Pays::Yes))]
		pub fn upgrade_registration(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			proof: ProofOfAttendance<T::Signature, T::AccountId>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin.clone())?;
			let current_phase = <pallet_encointer_scheduler::Pallet<T>>::current_phase();
			ensure!(
				<pallet_encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			ensure!(
				CeremonyPhaseType::is_registering_or_attesting(&current_phase),
				Error::<T>::RegisteringOrAttestationPhaseRequired
			);

			let mut cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();

			if current_phase == CeremonyPhaseType::Attesting {
				cindex += 1
			};

			let participant_type = Self::get_participant_type((cid, cindex), &sender)
				.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;
			if participant_type == ParticipantType::Newbie {
				Self::remove_participant_from_registry(cid, cindex, &sender)?;
				Self::register_participant(origin, cid, Some(proof))?;
			} else {
				return Err(<Error<T>>::MustBeNewbieToUpgradeRegistration.into());
			}
			Ok(().into())
		}

		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::unregister_participant(), DispatchClass::Normal, Pays::Yes))]
		pub fn unregister_participant(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			maybe_reputation_community_ceremony: Option<CommunityCeremony>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let current_phase = <pallet_encointer_scheduler::Pallet<T>>::current_phase();
			ensure!(
				CeremonyPhaseType::is_registering_or_attesting(&current_phase),
				Error::<T>::RegisteringOrAttestationPhaseRequired
			);

			ensure!(
				<pallet_encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let mut cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();

			if current_phase == CeremonyPhaseType::Attesting {
				cindex += 1
			};

			let participant_type = Self::get_participant_type((cid, cindex), &sender)
				.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;
			if participant_type == ParticipantType::Reputable {
				let cc = maybe_reputation_community_ceremony
					.ok_or(<Error<T>>::ReputationCommunityCeremonyRequired)?;
				ensure!(
					cc.1 >= cindex.saturating_sub(Self::reputation_lifetime()),
					Error::<T>::ProofOutdated
				);

				ensure!(
					Self::participant_reputation(cc, &sender) == Reputation::VerifiedLinked(cindex),
					Error::<T>::ReputationMustBeLinked
				);

				<ParticipantReputation<T>>::insert(cc, &sender, Reputation::VerifiedUnlinked);
				<ParticipantReputation<T>>::remove((cid, cindex), &sender);

				// invalidate reputation cache
				sp_io::offchain_index::set(&reputation_cache_dirty_key(&sender), &true.encode());
			}
			Self::remove_participant_from_registry(cid, cindex, &sender)?;

			Ok(().into())
		}

		#[pallet::call_index(3)]
		#[pallet::weight((<T as Config>::WeightInfo::attest_attendees(), DispatchClass::Normal, Pays::Yes))]
		pub fn attest_attendees(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			number_of_participants_vote: u32,
			attestations: BoundedVec<T::AccountId, T::MaxAttestations>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				<pallet_encointer_scheduler::Pallet<T>>::current_phase()
					== CeremonyPhaseType::Attesting,
				Error::<T>::AttestationPhaseRequired
			);
			ensure!(
				<pallet_encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let (cindex, meetup_index, meetup_participants, _meetup_location, _meetup_time) =
				Self::gather_meetup_data(&cid, &sender)?;

			ensure!(
				attestations.len() < meetup_participants.len(),
				Error::<T>::TooManyAttestations
			);

			debug!(
				target: LOG,
				"{:?} attempts to submit {:?} attestations",
				sender,
				attestations.len()
			);

			<MeetupParticipantCountVote<T>>::insert(
				(cid, cindex),
				&sender,
				number_of_participants_vote,
			);

			Self::add_attestations_to_registry(
				sender,
				&cid,
				cindex,
				meetup_index,
				&meetup_participants,
				&attestations,
			)?;

			Ok(().into())
		}

		#[pallet::call_index(4)]
		#[pallet::weight((<T as Config>::WeightInfo::endorse_newcomer(), DispatchClass::Normal, Pays::Yes))]
		pub fn endorse_newcomer(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			newbie: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			ensure!(
				<pallet_encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let mut cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
			if <pallet_encointer_scheduler::Pallet<T>>::current_phase()
				!= CeremonyPhaseType::Registering
			{
				cindex += 1; //safe; cindex comes from within, will not overflow at +1/d
			}

			ensure!(
				Self::is_endorsed(&newbie, &(cid, cindex)).is_none(),
				Error::<T>::AlreadyEndorsed
			);

			Self::burn_newbie_tickets(cid, cindex, &sender)?;

			<Endorsees<T>>::insert((cid, cindex), newbie.clone(), ());
			let new_endorsee_count = Self::endorsee_count((cid, cindex))
				.checked_add(1)
				.ok_or(<Error<T>>::RegistryOverflow)?;
			<EndorseesCount<T>>::insert((cid, cindex), new_endorsee_count);

			if <NewbieIndex<T>>::contains_key((cid, cindex), &newbie) {
				Self::remove_participant_from_registry(cid, cindex, &newbie)?;
				Self::register(cid, cindex, &newbie, false)?;
			}

			debug!(target: LOG, "bootstrapper {:?} endorsed newbie: {:?}", sender, newbie);
			Self::deposit_event(Event::EndorsedParticipant(cid, sender, newbie));

			Ok(().into())
		}

		#[pallet::call_index(5)]
		#[pallet::weight((<T as Config>::WeightInfo::claim_rewards(), DispatchClass::Normal, Pays::Yes))]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			maybe_meetup_index: Option<MeetupIndexType>,
		) -> DispatchResultWithPostInfo {
			let participant = &ensure_signed(origin)?;

			let current_phase = <pallet_encointer_scheduler::Pallet<T>>::current_phase();
			let mut cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
			match current_phase {
				CeremonyPhaseType::Registering => cindex -= 1,
				CeremonyPhaseType::Attesting => (),
				CeremonyPhaseType::Assigning => {
					return Err(<Error<T>>::WrongPhaseForClaimingRewards.into())
				},
			}

			let meetup_index = match maybe_meetup_index {
				Some(index) => index,
				None => Self::get_meetup_index((cid, cindex), participant)
					.ok_or(<Error<T>>::ParticipantIsNotRegistered)?,
			};

			if <IssuedRewards<T>>::contains_key((cid, cindex), meetup_index) {
				return Err(<Error<T>>::RewardsAlreadyIssued.into());
			}
			info!(
				target: LOG,
				"validating meetup {:?} for cid {:?} triggered by {:?}",
				meetup_index,
				&cid,
				participant
			);

			//gather all data
			let meetup_participants = Self::get_meetup_participants((cid, cindex), meetup_index)?;
			let (participant_votes, participant_attestations) =
				Self::gather_meetup_validation_data(cid, cindex, meetup_participants.clone());

			// initialize an array of local participant indices that are eligible for the reward
			// indices will be deleted in the following based on various rules
			let mut participants_eligible_for_rewards: Vec<usize> =
				(0..meetup_participants.len()).collect();

			let attestation_threshold_fn =
				|i: usize| max(if i > 5 { i.saturating_sub(2) } else { i.saturating_sub(1) }, 1);
			let participant_judgements = match get_participant_judgements(
				&participants_eligible_for_rewards,
				&participant_votes,
				&participant_attestations,
				attestation_threshold_fn,
			) {
				Ok(participant_judgements) => participant_judgements,
				// handle errors
				Err(err) => {
					let (error, meetup_result) = match err {
						MeetupValidationError::BallotEmpty => {
							debug!(
								target: LOG,
								"ballot empty for meetup {:?}, cid: {:?}", meetup_index, cid
							);
							(
								Err(<Error<T>>::VotesNotDependable.into()),
								MeetupResult::VotesNotDependable,
							)
						},
						MeetupValidationError::NoDependableVote => {
							debug!(
								target: LOG,
								"ballot doesn't reach dependable majority for meetup {:?}, cid: {:?}",
								meetup_index,
								cid
							);
							(
								Err(<Error<T>>::VotesNotDependable.into()),
								MeetupResult::VotesNotDependable,
							)
						},
						MeetupValidationError::IndexOutOfBounds => {
							debug!(
								target: LOG,
								"index out of bounds for meetup {:?}, cid: {:?}", meetup_index, cid
							);
							(
								Err(<Error<T>>::MeetupValidationIndexOutOfBounds.into()),
								MeetupResult::MeetupValidationIndexOutOfBounds,
							)
						},
					};
					// only mark issuance as complete in registering phase
					// because in attesting phase there could be a failing early payout attempt
					if current_phase == CeremonyPhaseType::Registering {
						info!(target: LOG, "marking issuance as completed for failed meetup.");

						<IssuedRewards<T>>::insert((cid, cindex), meetup_index, meetup_result);
						Self::deposit_event(Event::MeetupEvaluated(
							cid,
							meetup_index,
							meetup_result,
						));
						return Ok(Pays::No.into());
					} else {
						return error;
					}
				},
			};
			if current_phase == CeremonyPhaseType::Attesting
				&& !participant_judgements.early_rewards_possible
			{
				debug!(
					target: LOG,
					"early rewards not possible for meetup {:?}, cid: {:?}", meetup_index, cid
				);
				return Err(<Error<T>>::EarlyRewardsNotPossible.into());
			}
			participants_eligible_for_rewards = participant_judgements.legit;
			// emit events
			for p in participant_judgements.excluded {
				let participant = meetup_participants
					.get(p.index)
					.ok_or(Error::<T>::MeetupValidationIndexOutOfBounds)?
					.clone();
				Self::deposit_event(Event::NoReward {
					cid,
					cindex,
					meetup_index,
					account: participant,
					reason: p.reason,
				});
			}

			Self::issue_rewards(
				cid,
				cindex,
				meetup_index,
				meetup_participants,
				participants_eligible_for_rewards,
			)?;
			Ok(Pays::No.into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight((<T as Config>::WeightInfo::set_inactivity_timeout(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_inactivity_timeout(
			origin: OriginFor<T>,
			inactivity_timeout: InactivityTimeoutType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			Self::do_set_inactivity_timeout(inactivity_timeout)
		}

		#[pallet::call_index(7)]
		#[pallet::weight((<T as Config>::WeightInfo::set_endorsement_tickets_per_bootstrapper(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_endorsement_tickets_per_bootstrapper(
			origin: OriginFor<T>,
			endorsement_tickets_per_bootstrapper: EndorsementTicketsType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<EndorsementTicketsPerBootstrapper<T>>::put(endorsement_tickets_per_bootstrapper);
			info!(
				target: LOG,
				"set endorsement tickets per bootstrapper to {}",
				endorsement_tickets_per_bootstrapper
			);
			Self::deposit_event(Event::EndorsementTicketsPerBootstrapperUpdated(
				endorsement_tickets_per_bootstrapper,
			));
			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight((<T as Config>::WeightInfo::set_endorsement_tickets_per_reputable(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_endorsement_tickets_per_reputable(
			origin: OriginFor<T>,
			endorsement_tickets_per_reputable: EndorsementTicketsType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<EndorsementTicketsPerReputable<T>>::put(endorsement_tickets_per_reputable);
			info!(
				target: LOG,
				"set endorsement tickets per reputable to {}", endorsement_tickets_per_reputable
			);
			Self::deposit_event(Event::EndorsementTicketsPerReputableUpdated(
				endorsement_tickets_per_reputable,
			));
			Ok(().into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight((<T as Config>::WeightInfo::set_reputation_lifetime(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_reputation_lifetime(
			origin: OriginFor<T>,
			reputation_lifetime: ReputationLifetimeType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<ReputationLifetime<T>>::put(reputation_lifetime);
			info!(target: LOG, "set reputation lifetime to {}", reputation_lifetime);
			Self::deposit_event(Event::ReputationLifetimeUpdated(reputation_lifetime));
			Ok(().into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight((<T as Config>::WeightInfo::set_meetup_time_offset(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_meetup_time_offset(
			origin: OriginFor<T>,
			meetup_time_offset: MeetupTimeOffsetType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			if <pallet_encointer_scheduler::Pallet<T>>::current_phase()
				!= CeremonyPhaseType::Registering
			{
				return Err(<Error<T>>::WrongPhaseForChangingMeetupTimeOffset.into());
			}

			// Meetup time offset needs to be in [-8h, 8h]
			if meetup_time_offset.abs() > 8 * 3600 * 1000 {
				return Err(<Error<T>>::InvalidMeetupTimeOffset.into());
			}

			<MeetupTimeOffset<T>>::put(meetup_time_offset);
			info!(target: LOG, "set meetup time offset to {} ms", meetup_time_offset);
			Self::deposit_event(Event::MeetupTimeOffsetUpdated(meetup_time_offset));
			Ok(().into())
		}

		#[pallet::call_index(11)]
		#[pallet::weight((<T as Config>::WeightInfo::set_time_tolerance(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_time_tolerance(
			origin: OriginFor<T>,
			time_tolerance: T::Moment,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<TimeTolerance<T>>::put(time_tolerance);
			info!(target: LOG, "set meetup time tolerance to {:?}", time_tolerance);
			Self::deposit_event(Event::TimeToleranceUpdated(time_tolerance));
			Ok(().into())
		}

		#[pallet::call_index(12)]
		#[pallet::weight((<T as Config>::WeightInfo::set_location_tolerance(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_location_tolerance(
			origin: OriginFor<T>,
			location_tolerance: u32,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<LocationTolerance<T>>::put(location_tolerance);
			info!(target: LOG, "set meetup location tolerance to {}", location_tolerance);
			Self::deposit_event(Event::LocationToleranceUpdated(location_tolerance));
			Ok(().into())
		}
		#[pallet::call_index(13)]
		#[pallet::weight((<T as Config>::WeightInfo::purge_community_ceremony(), DispatchClass::Normal, Pays::Yes))]
		pub fn purge_community_ceremony(
			origin: OriginFor<T>,
			community_ceremony: CommunityCeremony,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;

			Self::purge_community_ceremony_internal(community_ceremony);

			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Participant registered for next ceremony [community, participant type, who]
		ParticipantRegistered(CommunityIdentifier, ParticipantType, T::AccountId),
		/// A bootstrapper (first accountid) has endorsed a participant (second accountid) who can now register as endorsee for this ceremony
		EndorsedParticipant(CommunityIdentifier, T::AccountId, T::AccountId),
		/// A participant has registered N attestations for fellow meetup participants
		AttestationsRegistered(CommunityIdentifier, MeetupIndexType, u32, T::AccountId),
		/// rewards have been claimed and issued successfully for N participants for their meetup at the previous ceremony
		RewardsIssued(CommunityIdentifier, MeetupIndexType, MeetupParticipantIndexType),
		/// inactivity timeout has changed. affects how many ceremony cycles a community can be idle before getting purged
		InactivityTimeoutUpdated(InactivityTimeoutType),
		/// The number of endorsement tickets which bootstrappers can give out has changed
		EndorsementTicketsPerBootstrapperUpdated(EndorsementTicketsType),
		/// The number of endorsement tickets which bootstrappers can give out has changed
		EndorsementTicketsPerReputableUpdated(EndorsementTicketsType),
		/// reputation lifetime has changed. After this many ceremony cycles, reputations is outdated
		ReputationLifetimeUpdated(ReputationLifetimeType),
		/// meetup time offset has changed. affects the exact time the upcoming ceremony meetups will take place
		MeetupTimeOffsetUpdated(MeetupTimeOffsetType),
		/// meetup time tolerance has changed
		TimeToleranceUpdated(T::Moment),
		/// meetup location tolerance changed [m]
		LocationToleranceUpdated(u32),
		/// the registry for given ceremony index and community has been purged
		CommunityCeremonyHistoryPurged(CommunityIdentifier, CeremonyIndexType),

		NoReward {
			cid: CommunityIdentifier,
			cindex: CeremonyIndexType,
			meetup_index: MeetupIndexType,
			account: T::AccountId,
			reason: ExclusionReason,
		},

		/// The inactivity counter of a community has been increased
		InactivityCounterUpdated(CommunityIdentifier, u32),

		/// Result of the meetup at the previous ceremony
		MeetupEvaluated(CommunityIdentifier, MeetupIndexType, MeetupResult),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// the participant is already registered
		ParticipantAlreadyRegistered,
		/// verification of signature of attendee failed
		BadProofOfAttendanceSignature,
		/// verification of signature of attendee failed
		BadAttendeeSignature,
		/// meetup location was not found
		MeetupLocationNotFound,
		/// meetup time calculation failed
		MeetupTimeCalculationError,
		/// no valid claims were supplied
		NoValidAttestations,
		/// the action can only be performed during ATTESTING phase
		AttestationPhaseRequired,
		/// the action can only be performed during REGISTERING or ATTESTING phase
		RegisteringOrAttestationPhaseRequired,
		/// CommunityIdentifier not found
		InexistentCommunity,
		/// proof is outdated
		ProofOutdated,
		/// proof is acausal
		ProofAcausal,
		/// supplied proof is not proving sender
		WrongProofSubject,
		/// former attendance has not been verified or has already been linked to other account
		AttendanceUnverifiedOrAlreadyUsed,
		/// can't have more attestations than other meetup participants
		TooManyAttestations,
		/// sender has run out of newbie tickets
		NoMoreNewbieTickets,
		/// newbie is already endorsed
		AlreadyEndorsed,
		/// Participant is not registered
		ParticipantIsNotRegistered,
		/// No locations are available for assigning participants
		NoLocationsAvailable,
		/// Trying to issue rewards in a phase that is not REGISTERING
		WrongPhaseForClaimingRewards,
		/// Trying to issue rewards for a meetup for which UBI was already issued
		RewardsAlreadyIssued,
		/// Trying to claim UBI for a meetup where votes are not dependable
		VotesNotDependable,
		/// Overflow adding user to registry
		RegistryOverflow,
		/// CheckedMath operation error
		CheckedMath,
		/// Only Bootstrappers are allowed to be registered at this time
		OnlyBootstrappers,
		/// MeetupTimeOffset can only be changed during registering
		WrongPhaseForChangingMeetupTimeOffset,
		/// MeetupTimeOffset needs to be in [-8h, 8h]
		InvalidMeetupTimeOffset,
		/// the history for given ceremony index and community has been purged
		CommunityCeremonyHistoryPurged,
		/// Unregistering can only be performed during the registering phase
		WrongPhaseForUnregistering,
		/// Error while finding meetup participants
		GetMeetupParticipantsError,
		/// index out of bounds while validating the meetup
		MeetupValidationIndexOutOfBounds,
		/// Not possible to pay rewards in attestations phase
		EarlyRewardsNotPossible,
		/// Only newbies can upgrade their registration
		MustBeNewbieToUpgradeRegistration,
		/// To unregister as a reputable you need to provide a provide a community ceremony where you have a linked reputation
		ReputationCommunityCeremonyRequired,
		/// In order to unregister a reputable, the provided reputation must be linked
		ReputationMustBeLinked,
		/// Meetup Index > Meetup Count or < 1
		InvalidMeetupIndex,
		/// BoundedVec bound reached
		TooManyAttestationsInBoundedVec,
	}

	#[pallet::storage]
	#[pallet::getter(fn bootstrapper_newbie_tickets)]
	pub(super) type BurnedBootstrapperNewbieTickets<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		EndorsementTicketsType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reputable_newbie_tickets)]
	pub(super) type BurnedReputableNewbieTickets<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		EndorsementTicketsType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bootstrapper_registry)]
	pub(super) type BootstrapperRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		ParticipantIndexType,
		T::AccountId,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bootstrapper_index)]
	pub(super) type BootstrapperIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		ParticipantIndexType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bootstrapper_count)]
	pub(super) type BootstrapperCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ParticipantIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn reputable_registry)]
	pub(super) type ReputableRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		ParticipantIndexType,
		T::AccountId,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reputable_index)]
	pub(super) type ReputableIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		ParticipantIndexType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reputable_count)]
	pub(super) type ReputableCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ParticipantIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn endorsee_registry)]
	pub(super) type EndorseeRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		ParticipantIndexType,
		T::AccountId,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn endorsee_index)]
	pub(super) type EndorseeIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		ParticipantIndexType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn endorsee_count)]
	pub(super) type EndorseeCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ParticipantIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn newbie_registry)]
	pub(super) type NewbieRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		ParticipantIndexType,
		T::AccountId,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn newbie_index)]
	pub(super) type NewbieIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		ParticipantIndexType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn newbie_count)]
	pub(super) type NewbieCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ParticipantIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn assignment_counts)]
	pub(super) type AssignmentCounts<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, AssignmentCount, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn assignments)]
	pub(super) type Assignments<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, Assignment, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn participant_reputation)]
	pub(super) type ParticipantReputation<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		Reputation,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn reputation_count)]
	pub(super) type ReputationCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ReputationCountType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn global_reputation_count)]
	pub(super) type GlobalReputationCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CeremonyIndexType, ReputationCountType, ValueQuery>;

	/// Accounts that have been endorsed by a reputable or a bootstrapper.
	///
	/// This is not the same as `EndorseeRegistry`, which contains the `Endorsees` who
	/// have registered for a meetup.
	#[pallet::storage]
	#[pallet::getter(fn endorsees)]
	pub(super) type Endorsees<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		(),
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn endorsees_count)]
	pub(super) type EndorseesCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, ParticipantIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn meetup_count)]
	pub(super) type MeetupCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, MeetupIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn attestation_registry)]
	pub(super) type AttestationRegistry<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		AttestationIndexType,
		BoundedVec<T::AccountId, T::MaxAttestations>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn attestation_index)]
	pub(super) type AttestationIndex<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		AttestationIndexType,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn attestation_count)]
	pub(super) type AttestationCount<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityCeremony, AttestationIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn meetup_participant_count_vote)]
	pub(super) type MeetupParticipantCountVote<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		T::AccountId,
		u32,
		ValueQuery,
	>;

	/// the default UBI for a ceremony attendee if no community specific value is set.
	#[pallet::storage]
	#[pallet::getter(fn ceremony_reward)]
	pub(super) type CeremonyReward<T: Config> = StorageValue<_, BalanceType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn location_tolerance)]
	pub(super) type LocationTolerance<T: Config> = StorageValue<_, u32, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn time_tolerance)]
	pub(super) type TimeTolerance<T: Config> = StorageValue<_, T::Moment, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn issued_rewards)]
	pub(super) type IssuedRewards<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		MeetupIndexType,
		MeetupResult,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn inactivity_counters)]
	pub(super) type InactivityCounters<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdentifier, u32>;

	/// The number of ceremony cycles a community can skip ceremonies before it gets purged
	#[pallet::storage]
	#[pallet::getter(fn inactivity_timeout)]
	pub(super) type InactivityTimeout<T: Config> =
		StorageValue<_, InactivityTimeoutType, ValueQuery>;

	/// The number of newbies a bootstrapper can endorse to accelerate community growth
	#[pallet::storage]
	#[pallet::getter(fn endorsement_tickets_per_bootstrapper)]
	pub(super) type EndorsementTicketsPerBootstrapper<T: Config> =
		StorageValue<_, EndorsementTicketsType, ValueQuery>;

	/// The number of newbies a reputable can endorse per cycle to accelerate community growth
	#[pallet::storage]
	#[pallet::getter(fn endorsement_tickets_per_reputable)]
	pub(super) type EndorsementTicketsPerReputable<T: Config> =
		StorageValue<_, EndorsementTicketsType, ValueQuery>;

	/// The number of ceremony cycles that a participant's reputation is valid for
	#[pallet::storage]
	#[pallet::getter(fn reputation_lifetime)]
	pub(super) type ReputationLifetime<T: Config> =
		StorageValue<_, ReputationLifetimeType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn meetup_time_offset)]
	pub(super) type MeetupTimeOffset<T: Config> = StorageValue<_, MeetupTimeOffsetType, ValueQuery>;

	#[derive(frame_support::DefaultNoBound)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		#[doc = " the default UBI for a ceremony attendee if no community specific value is set."]
		pub ceremony_reward: BalanceType,
		pub location_tolerance: u32,
		pub time_tolerance: T::Moment,
		pub inactivity_timeout: InactivityTimeoutType,
		pub endorsement_tickets_per_bootstrapper: EndorsementTicketsType,
		pub endorsement_tickets_per_reputable: EndorsementTicketsType,
		pub reputation_lifetime: ReputationLifetimeType,
		pub meetup_time_offset: MeetupTimeOffsetType,
		#[serde(skip)]
		pub _config: sp_std::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		fn build(&self) {
			<CeremonyReward<T>>::put(self.ceremony_reward);
			<LocationTolerance<T>>::put(self.location_tolerance);
			<TimeTolerance<T>>::put(self.time_tolerance);
			<InactivityTimeout<T>>::put(self.inactivity_timeout);
			<EndorsementTicketsPerBootstrapper<T>>::put(self.endorsement_tickets_per_bootstrapper);
			<EndorsementTicketsPerReputable<T>>::put(self.endorsement_tickets_per_reputable);
			<ReputationLifetime<T>>::put(self.reputation_lifetime);
			<MeetupTimeOffset<T>>::put(self.meetup_time_offset);
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_set_inactivity_timeout(
		inactivity_timeout: InactivityTimeoutType,
	) -> DispatchResultWithPostInfo {
		<InactivityTimeout<T>>::put(inactivity_timeout);
		info!(target: LOG, "set inactivity timeout to {}", inactivity_timeout);
		Self::deposit_event(Event::InactivityTimeoutUpdated(inactivity_timeout));
		Ok(().into())
	}

	pub fn get_reputations(
		account: &T::AccountId,
	) -> Vec<(CeremonyIndexType, CommunityReputation)> {
		ParticipantReputation::<T>::iter()
			.filter(|t| &t.1 == account)
			.map(|t| (t.0 .1, CommunityReputation::new(t.0 .0, t.2)))
			.collect()
	}

	pub fn get_aggregated_account_data(
		cid: CommunityIdentifier,
		account: &T::AccountId,
	) -> AggregatedAccountData<T::AccountId, T::Moment> {
		let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
		let aggregated_account_data_global = AggregatedAccountDataGlobal {
			ceremony_phase: <pallet_encointer_scheduler::Pallet<T>>::current_phase(),
			ceremony_index: cindex,
		};

		let aggregated_account_data_personal: Option<
			AggregatedAccountDataPersonal<T::AccountId, T::Moment>,
		>;

		// check if the participant is registered. if not the entire personal field will be None.
		if let Some(participant_type) = Self::get_participant_type((cid, cindex), account) {
			let mut meetup_location_index: Option<MeetupIndexType> = None;
			let mut meetup_time: Option<T::Moment> = None;
			let mut meetup_registry: Option<Vec<T::AccountId>> = None;
			let mut meetup_index: Option<MeetupIndexType> = None;

			// check if the participant is already assigned to a meetup
			if let Some(participant_meetup_index) = Self::get_meetup_index((cid, cindex), account) {
				meetup_index = Some(participant_meetup_index);
				let locations = <pallet_encointer_communities::Pallet<T>>::get_locations(&cid);
				let location_assignment_params = Self::assignments((cid, cindex)).locations;

				meetup_location_index = get_meetup_location_index(
					participant_meetup_index,
					&locations,
					location_assignment_params,
				);
				if let Some(location) =
					Self::get_meetup_location((cid, cindex), participant_meetup_index)
				{
					meetup_time = Self::get_meetup_time(location);
				}

				meetup_registry =
					Self::get_meetup_participants((cid, cindex), participant_meetup_index).ok();
			}

			aggregated_account_data_personal =
				Some(AggregatedAccountDataPersonal::<T::AccountId, T::Moment> {
					participant_type,
					meetup_index,
					meetup_location_index,
					meetup_time,
					meetup_registry,
				});
		} else {
			aggregated_account_data_personal = None;
		}
		AggregatedAccountData::<T::AccountId, T::Moment> {
			global: aggregated_account_data_global,
			personal: aggregated_account_data_personal,
		}
	}

	pub fn get_ceremony_info() -> CeremonyInfo {
		CeremonyInfo {
			ceremony_phase: <pallet_encointer_scheduler::Pallet<T>>::current_phase(),
			ceremony_index: <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index(),
		}
	}

	fn register(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
		is_reputable: bool,
	) -> Result<ParticipantType, Error<T>> {
		let participant_type =
			if <pallet_encointer_communities::Pallet<T>>::bootstrappers(cid).contains(sender) {
				let participant_index = <BootstrapperCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<BootstrapperRegistry<T>>::insert((cid, cindex), participant_index, sender);
				<BootstrapperIndex<T>>::insert((cid, cindex), sender, participant_index);
				<BootstrapperCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Bootstrapper
			} else if <pallet_encointer_balances::Pallet<T>>::total_issuance(cid) <= 0 {
				return Err(Error::<T>::OnlyBootstrappers);
			} else if is_reputable {
				let participant_index = <ReputableCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<ReputableRegistry<T>>::insert((cid, cindex), participant_index, sender);
				<ReputableIndex<T>>::insert((cid, cindex), sender, participant_index);
				<ReputableCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Reputable
			} else if let Some(endorsed_cindex) = Self::is_endorsed(sender, &(cid, cindex)) {
				let participant_index = <EndorseeCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<Endorsees<T>>::remove((cid, endorsed_cindex), sender);
				<EndorseeRegistry<T>>::insert((cid, cindex), participant_index, sender);
				<EndorseeIndex<T>>::insert((cid, cindex), sender, participant_index);
				<EndorseeCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Endorsee
			} else {
				let participant_index = <NewbieCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<NewbieRegistry<T>>::insert((cid, cindex), participant_index, sender);
				<NewbieIndex<T>>::insert((cid, cindex), sender, participant_index);
				<NewbieCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Newbie
			};
		Ok(participant_type)
	}

	/// removes a participant from the registry maintaining continuous indices
	fn remove_participant_from_registry(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		participant: &T::AccountId,
	) -> Result<(), Error<T>> {
		if <pallet_encointer_scheduler::Pallet<T>>::current_phase()
			!= CeremonyPhaseType::Registering
		{
			return Err(<Error<T>>::WrongPhaseForUnregistering);
		}

		let participant_type = Self::get_participant_type((cid, cindex), participant)
			.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;
		match participant_type {
			ParticipantType::Bootstrapper => {
				storage_helper::remove_participant_from_registry::<
					BootstrapperIndex<T>,
					BootstrapperRegistry<T>,
					BootstrapperCount<T>,
					T::AccountId,
				>(cid, cindex, participant);
			},
			ParticipantType::Reputable => {
				storage_helper::remove_participant_from_registry::<
					ReputableIndex<T>,
					ReputableRegistry<T>,
					ReputableCount<T>,
					T::AccountId,
				>(cid, cindex, participant);
			},
			ParticipantType::Endorsee => {
				storage_helper::remove_participant_from_registry::<
					EndorseeIndex<T>,
					EndorseeRegistry<T>,
					EndorseeCount<T>,
					T::AccountId,
				>(cid, cindex, participant);
			},
			ParticipantType::Newbie => {
				storage_helper::remove_participant_from_registry::<
					NewbieIndex<T>,
					NewbieRegistry<T>,
					NewbieCount<T>,
					T::AccountId,
				>(cid, cindex, participant);
			},
		}

		Ok(())
	}

	fn is_registered(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
	) -> bool {
		<BootstrapperIndex<T>>::contains_key((cid, cindex), sender)
			|| <ReputableIndex<T>>::contains_key((cid, cindex), sender)
			|| <EndorseeIndex<T>>::contains_key((cid, cindex), sender)
			|| <NewbieIndex<T>>::contains_key((cid, cindex), sender)
	}

	/// Will burn the `sender`'s newbie tickets if he has some.
	///
	/// First we try to use the the reputable tickets because they refill with new reputation, and
	/// then we try to use the bootstrapper tickets.
	fn burn_newbie_tickets(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
	) -> Result<(), Error<T>> {
		if Self::has_reputation(sender, &cid)
			&& <BurnedReputableNewbieTickets<T>>::get((cid, cindex), sender)
				< Self::endorsement_tickets_per_reputable()
		{
			// safe; limited by AMOUNT_NEWBIE_TICKETS
			<BurnedReputableNewbieTickets<T>>::mutate((cid, cindex), sender, |b| *b += 1);
			return Ok(());
		}

		if <pallet_encointer_communities::Pallet<T>>::bootstrappers(cid).contains(sender)
			&& <BurnedBootstrapperNewbieTickets<T>>::get(cid, sender)
				< Self::endorsement_tickets_per_bootstrapper()
		{
			// safe; limited by AMOUNT_NEWBIE_TICKETS
			<BurnedBootstrapperNewbieTickets<T>>::mutate(cid, sender, |b| *b += 1);
			return Ok(());
		}

		Err(Error::<T>::NoMoreNewbieTickets)
	}

	#[allow(deprecated)]
	fn purge_community_ceremony_internal(cc: CommunityCeremony) {
		let cid = cc.1;
		let cindex = cc.0;

		info!(target: LOG, "purging ceremony index {} history for {:?}", cindex, cid);

		<BootstrapperRegistry<T>>::remove_prefix(cc, None);
		<BootstrapperIndex<T>>::remove_prefix(cc, None);
		<BootstrapperCount<T>>::remove(cc);

		<ReputableRegistry<T>>::remove_prefix(cc, None);
		<ReputableIndex<T>>::remove_prefix(cc, None);
		<ReputableCount<T>>::remove(cc);

		<EndorseeRegistry<T>>::remove_prefix(cc, None);
		<EndorseeIndex<T>>::remove_prefix(cc, None);
		<EndorseeCount<T>>::remove(cc);

		<NewbieRegistry<T>>::remove_prefix(cc, None);
		<NewbieIndex<T>>::remove_prefix(cc, None);
		<NewbieCount<T>>::remove(cc);

		<AssignmentCounts<T>>::remove(cc);

		Assignments::<T>::remove(cc);

		<ParticipantReputation<T>>::remove_prefix(cc, None);
		<ReputationCount<T>>::remove(cc);
		<GlobalReputationCount<T>>::remove(cc.1);

		<Endorsees<T>>::remove_prefix(cc, None);
		<EndorseesCount<T>>::remove(cc);
		<MeetupCount<T>>::remove(cc);

		<AttestationRegistry<T>>::remove_prefix(cc, None);
		<AttestationIndex<T>>::remove_prefix(cc, None);
		<AttestationCount<T>>::remove(cc);

		<MeetupParticipantCountVote<T>>::remove_prefix(cc, None);
		<IssuedRewards<T>>::remove_prefix(cc, None);
		<BurnedReputableNewbieTickets<T>>::remove_prefix(cc, None);

		Self::deposit_event(Event::CommunityCeremonyHistoryPurged(cindex, cid));
	}

	fn purge_registry(cindex: CeremonyIndexType) {
		let cids = <pallet_encointer_communities::Pallet<T>>::community_identifiers();
		for cid in cids.into_iter() {
			Self::purge_community_ceremony_internal((cid, cindex));
		}
		debug!(target: LOG, "purged registry for ceremony {}", cindex);
	}

	fn generate_meetup_assignment_params(
		community_ceremony: CommunityCeremony,
		random_source: &mut RandomNumberGenerator<T::Hashing>,
	) -> DispatchResult {
		info!(
			target: LOG,
			"generating meetup assignment params for cid: {:?}", community_ceremony.0
		);
		let meetup_multiplier = T::MeetupSizeTarget::get();
		let assignment_allowance =
			Self::compute_assignment_allowance(community_ceremony, meetup_multiplier)?;
		let num_meetups = checked_ceil_division(
			assignment_allowance.get_number_of_participants(),
			meetup_multiplier,
		)
		.ok_or(Error::<T>::CheckedMath)?;
		if assignment_allowance.get_number_of_participants() < T::MeetupMinSize::get() {
			info!(
				target: LOG,
				"less than 3 participants available for a meetup. will not assign any meetups for cid {:?}",
				community_ceremony.0
			);
			return Ok(());
		}
		info!(target: LOG, "assigning {:} meetups for cid {:?}", num_meetups, community_ceremony.0);

		<Assignments<T>>::insert(
			community_ceremony,
			Assignment {
				bootstrappers_reputables: generate_assignment_function_params(
					assignment_allowance.bootstrappers + assignment_allowance.reputables,
					num_meetups,
					random_source,
				),
				endorsees: generate_assignment_function_params(
					assignment_allowance.endorsees,
					num_meetups,
					random_source,
				),
				newbies: generate_assignment_function_params(
					assignment_allowance.newbies,
					num_meetups,
					random_source,
				),
				locations: Self::generate_location_assignment_params(
					community_ceremony,
					random_source,
				),
			},
		);

		<AssignmentCounts<T>>::insert(community_ceremony, assignment_allowance);
		<MeetupCount<T>>::insert(community_ceremony, num_meetups);
		Ok(())
	}

	fn generate_location_assignment_params(
		community_ceremony: CommunityCeremony,
		random_source: &mut RandomNumberGenerator<T::Hashing>,
	) -> AssignmentParams {
		let num_locations =
			<pallet_encointer_communities::Pallet<T>>::get_locations(&community_ceremony.0).len()
				as u64;

		AssignmentParams {
			m: num_locations,
			s1: find_random_coprime_below(num_locations, random_source),
			s2: find_prime_below(num_locations),
		}
	}

	fn compute_assignment_allowance(
		community_ceremony: CommunityCeremony,
		meetup_multiplier: u64,
	) -> Result<AssignmentCount, Error<T>> {
		let num_locations =
			<pallet_encointer_communities::Pallet<T>>::get_locations(&community_ceremony.0).len()
				as u64;
		debug!(
			target: LOG,
			"Number of locations for cid {:?} is {:?}", community_ceremony.0, num_locations
		);
		if num_locations == 0 {
			return Err(<Error<T>>::NoLocationsAvailable);
		}

		let num_registered_bootstrappers = Self::bootstrapper_count(community_ceremony);
		let num_registered_reputables = Self::reputable_count(community_ceremony);
		let num_registered_endorsees = Self::endorsee_count(community_ceremony);
		let num_registered_newbies = Self::newbie_count(community_ceremony);
		debug!(
			target: LOG,
			"Number of registered bootstrappers {:?}, endorsees {:?}, reputables {:?}, newbies {:?}",
			num_registered_bootstrappers, num_registered_endorsees, num_registered_reputables,
			num_registered_newbies
		);

		let max_num_meetups = min(
			num_locations,
			find_prime_below(num_registered_bootstrappers + num_registered_reputables),
		);

		//safe; number of assigned bootstrappers <= max_num_meetups <=num_assigned_bootstrappers + num_reputables
		let mut seats_left =
			max_num_meetups.checked_mul(meetup_multiplier).ok_or(Error::<T>::CheckedMath)?
				- num_registered_bootstrappers;

		let num_assigned_reputables = min(num_registered_reputables, seats_left);
		seats_left -= num_assigned_reputables; //safe; given by minimum above

		let num_assigned_endorsees = min(num_registered_endorsees, seats_left);
		seats_left -= num_assigned_endorsees; //safe; given by minimum above

		let num_assigned_newbies = min(
			min(num_registered_newbies, seats_left),
			(num_registered_bootstrappers + num_assigned_reputables + num_assigned_endorsees)
				/ T::MeetupNewbieLimitDivider::get(), //safe; sum equals total
		);
		info!(
			target: LOG,
			"Number of assigned bootstrappers {:?}, endorsees {:?}, reputables {:?}, newbies {:?}",
			num_registered_bootstrappers,
			num_assigned_endorsees,
			num_assigned_reputables,
			num_assigned_newbies
		);
		Ok(AssignmentCount {
			bootstrappers: num_registered_bootstrappers,
			reputables: num_assigned_reputables,
			endorsees: num_assigned_endorsees,
			newbies: num_assigned_newbies,
		})
	}

	fn update_inactivity_counters(
		cindex: u32,
		inactivity_timeout: u32,
		cids: Vec<CommunityIdentifier>,
	) -> Vec<CommunityIdentifier> {
		let mut inactives = vec![];
		for cid in cids {
			if <IssuedRewards<T>>::iter_prefix_values((cid, cindex)).next().is_some() {
				<InactivityCounters<T>>::insert(cid, 0);
				Self::deposit_event(Event::InactivityCounterUpdated(cid, 0));
			} else {
				let current = Self::inactivity_counters(cid).unwrap_or(0);
				if current >= inactivity_timeout {
					inactives.push(cid);
				} else {
					let new_counter = current + 1;
					<InactivityCounters<T>>::insert(cid, new_counter);
					Self::deposit_event(Event::InactivityCounterUpdated(cid, new_counter));
				}
			}
		}
		inactives
	}

	fn purge_community(cid: CommunityIdentifier) {
		let current = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
		let reputation_lifetime = Self::reputation_lifetime();

		for cindex in current.saturating_sub(reputation_lifetime)..=current {
			Self::purge_community_ceremony_internal((cid, cindex));
		}

		<InactivityCounters<T>>::remove(cid);

		#[allow(deprecated)]
		<BurnedBootstrapperNewbieTickets<T>>::remove_prefix(cid, None);

		<pallet_encointer_communities::Pallet<T>>::remove_community(cid);
	}

	fn generate_all_meetup_assignment_params() {
		let cids = <pallet_encointer_communities::Pallet<T>>::community_identifiers();
		let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();

		// we don't need to pass a subject here, as this is only called once in a block.
		let mut random_source =
			RandomNumberGenerator::<T::Hashing>::new(T::RandomnessSource::random_seed().0);

		for cid in cids.iter() {
			if let Err(e) =
				Self::generate_meetup_assignment_params((*cid, cindex), &mut random_source)
			{
				error!(
					target: LOG,
					"Could not generate meetup assignment params for cid: {:?}. {:?}", cid, e
				);
			}
		}
	}

	fn get_participant_type(
		community_ceremony: CommunityCeremony,
		participant: &T::AccountId,
	) -> Option<ParticipantType> {
		if <BootstrapperIndex<T>>::contains_key(community_ceremony, participant) {
			return Some(ParticipantType::Bootstrapper);
		}
		if <ReputableIndex<T>>::contains_key(community_ceremony, participant) {
			return Some(ParticipantType::Reputable);
		}
		if <EndorseeIndex<T>>::contains_key(community_ceremony, participant) {
			return Some(ParticipantType::Endorsee);
		}
		if <NewbieIndex<T>>::contains_key(community_ceremony, participant) {
			return Some(ParticipantType::Newbie);
		}
		None
	}

	fn get_meetup_index(
		community_ceremony: CommunityCeremony,
		participant: &T::AccountId,
	) -> Option<MeetupIndexType> {
		let meetup_count = Self::meetup_count(community_ceremony);
		let assignment_count = Self::assignment_counts(community_ceremony);

		let assignment = Self::assignments(community_ceremony);

		let participant_type = Self::get_participant_type(community_ceremony, participant)?;

		let (participant_index, assignment_params) = match participant_type {
			ParticipantType::Bootstrapper => {
				let participant_index =
					Self::bootstrapper_index(community_ceremony, participant) - 1;
				if participant_index < assignment_count.bootstrappers {
					(participant_index, assignment.bootstrappers_reputables)
				} else {
					return None;
				}
			},
			ParticipantType::Reputable => {
				let participant_index = Self::reputable_index(community_ceremony, participant) - 1;
				if participant_index < assignment_count.reputables {
					(
						participant_index + assignment_count.bootstrappers,
						assignment.bootstrappers_reputables,
					)
				} else {
					return None;
				}
			},

			ParticipantType::Endorsee => {
				let participant_index = Self::endorsee_index(community_ceremony, participant) - 1;
				if participant_index < assignment_count.endorsees {
					(participant_index, assignment.endorsees)
				} else {
					return None;
				}
			},

			ParticipantType::Newbie => {
				let participant_index = Self::newbie_index(community_ceremony, participant) - 1;
				if participant_index < assignment_count.newbies {
					(participant_index, assignment.newbies)
				} else {
					return None;
				}
			},
		};

		meetup_index(participant_index, assignment_params, meetup_count)
	}

	fn get_meetup_participants(
		community_ceremony: CommunityCeremony,
		mut meetup_index: MeetupIndexType,
	) -> Result<Vec<T::AccountId>, Error<T>> {
		let mut result: Vec<T::AccountId> = vec![];
		let meetup_count = Self::meetup_count(community_ceremony);

		if meetup_index > meetup_count || meetup_index < 1 {
			error!(
				target: LOG,
				"Invalid meetup index {}, meetup_count is {}", meetup_index, meetup_count
			);
			return Err(<Error<T>>::InvalidMeetupIndex);
		}

		//safe; meetup index conversion from 1 based to 0 based
		meetup_index -= 1;

		let params = Self::assignments(community_ceremony);

		let assigned = Self::assignment_counts(community_ceremony);

		let bootstrappers_reputables = assignment_fn_inverse(
			meetup_index,
			params.bootstrappers_reputables,
			meetup_count,
			assigned.bootstrappers + assigned.reputables,
		)
		.ok_or(<Error<T>>::GetMeetupParticipantsError)?;
		for p in bootstrappers_reputables {
			if p < assigned.bootstrappers {
				//safe; small number per meetup
				match Self::bootstrapper_registry(community_ceremony, p + 1) {
					Some(bs) => result.push(bs),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Bootstrapper not found!!"
					),
				}
			} else if p < assigned.bootstrappers + assigned.reputables {
				//safe; small number per meetup
				match Self::reputable_registry(community_ceremony, p - assigned.bootstrappers + 1) {
					Some(r) => result.push(r),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Reputable not found!!"
					),
				};
			}
		}

		let endorsees =
			assignment_fn_inverse(meetup_index, params.endorsees, meetup_count, assigned.endorsees)
				.ok_or(<Error<T>>::GetMeetupParticipantsError)?;
		for p in endorsees {
			if p < assigned.endorsees {
				//safe; small number per meetup
				match Self::endorsee_registry(community_ceremony, p + 1) {
					Some(e) => result.push(e),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Endorsee not found!!"
					),
				};
			}
		}

		let newbies =
			assignment_fn_inverse(meetup_index, params.newbies, meetup_count, assigned.newbies)
				.ok_or(<Error<T>>::GetMeetupParticipantsError)?;
		for p in newbies {
			if p < assigned.newbies {
				//safe; small number per meetup
				match Self::newbie_registry(community_ceremony, p + 1) {
					Some(n) => result.push(n),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Newbie not found!!"
					),
				};
			}
		}

		Ok(result)
	}

	pub fn get_meetup_location(
		cc: CommunityCeremony,
		meetup_idx: MeetupIndexType,
	) -> Option<Location> {
		let locations = <pallet_encointer_communities::Pallet<T>>::get_locations(&cc.0);
		let assignment_params = Self::assignments(cc).locations;

		meetup_location(meetup_idx, locations, assignment_params)
	}

	// this function only works during ATTESTING, so we're keeping it for private use
	pub(crate) fn get_meetup_time(location: Location) -> Option<T::Moment> {
		if !(<pallet_encointer_scheduler::Pallet<T>>::current_phase()
			== CeremonyPhaseType::Attesting)
		{
			return None;
		}

		let duration =
			<pallet_encointer_scheduler::Pallet<T>>::phase_durations(CeremonyPhaseType::Attesting);
		let next = <pallet_encointer_scheduler::Pallet<T>>::next_phase_timestamp();
		let start = next - duration;

		Some(meetup_time::<T::Moment>(
			location,
			start,
			T::MomentsPerDay::get(),
			Self::meetup_time_offset(),
		))
	}

	/// Returns the community-specific nominal income if it is set. Otherwise returns the
	/// the ceremony reward defined in the genesis config.
	pub fn nominal_income(cid: &CommunityIdentifier) -> NominalIncome {
		pallet_encointer_communities::NominalIncome::<T>::try_get(cid)
			.unwrap_or_else(|_| Self::ceremony_reward())
	}

	fn issue_rewards(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		meetup_idx: MeetupIndexType,
		meetup_participants: Vec<T::AccountId>,
		participants_indices: Vec<usize>,
	) -> Result<(), Error<T>> {
		let reward = Self::nominal_income(&cid);
		let mut reputation_count = 0;
		for i in &participants_indices {
			let participant = &meetup_participants
				.get(*i)
				.ok_or(Error::<T>::MeetupValidationIndexOutOfBounds)?;
			trace!(target: LOG, "participant merits reward: {:?}", participant);

			if <pallet_encointer_balances::Pallet<T>>::issue(cid, participant, reward).is_ok() {
				<ParticipantReputation<T>>::insert(
					(&cid, cindex),
					participant,
					Reputation::VerifiedUnlinked,
				);
				reputation_count += 1;
			}
			sp_io::offchain_index::set(&reputation_cache_dirty_key(participant), &true.encode());
		}
		<ReputationCount<T>>::mutate((&cid, cindex), |b| *b += reputation_count); // safe, as reputation_count is limited by the number of locations available on earth
		<GlobalReputationCount<T>>::mutate(cindex, |b| *b += reputation_count); // safe, as reputation_count is limited by the number of locations available on earth

		<IssuedRewards<T>>::insert((cid, cindex), meetup_idx, MeetupResult::Ok);
		info!(target: LOG, "issuing rewards completed");

		Self::deposit_event(Event::RewardsIssued(
			cid,
			meetup_idx,
			participants_indices.len() as MeetupParticipantIndexType,
		));
		Ok(())
	}

	fn gather_meetup_validation_data(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		meetup_participants: Vec<T::AccountId>,
	) -> (Vec<u32>, Vec<Vec<usize>>) {
		let mut participant_votes: Vec<u32> = vec![];
		let mut participant_attestations: Vec<Vec<usize>> = vec![];

		// gather votes and attestations
		for participant in meetup_participants.iter() {
			let attestations = match Self::attestation_registry(
				(&cid, cindex),
				Self::attestation_index((cid, cindex), participant),
			) {
				Some(attestees) => attestees,
				None => Default::default(),
			};
			// convert AccountId to local index
			let attestation_indices = attestations
				.into_iter()
				.filter_map(|p| meetup_participants.iter().position(|s| *s == p))
				.collect();

			participant_attestations.push(attestation_indices);
			participant_votes.push(Self::meetup_participant_count_vote((cid, cindex), participant));
		}
		(participant_votes, participant_attestations)
	}

	fn gather_meetup_data(
		cid: &CommunityIdentifier,
		participant: &T::AccountId,
	) -> Result<MeetupData<T::AccountId, T::Moment>, Error<T>> {
		let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();

		let meetup_index = Self::get_meetup_index((*cid, cindex), participant)
			.ok_or(Error::<T>::ParticipantIsNotRegistered)?;
		let meetup_participants = Self::get_meetup_participants((*cid, cindex), meetup_index)?;

		let meetup_location = Self::get_meetup_location((*cid, cindex), meetup_index)
			.ok_or(Error::<T>::MeetupLocationNotFound)?;

		let meetup_time =
			Self::get_meetup_time(meetup_location).ok_or(Error::<T>::MeetupTimeCalculationError)?;

		Ok((cindex, meetup_index, meetup_participants, meetup_location, meetup_time))
	}

	fn add_attestations_to_registry(
		participant: T::AccountId,
		cid: &CommunityIdentifier,
		cindex: CeremonyIndexType,
		meetup_index: MeetupIndexType,
		meetup_participants: &[T::AccountId],
		attestations: &[T::AccountId],
	) -> Result<(), Error<T>> {
		let mut verified_attestees = vec![];
		for attestee in attestations.iter() {
			if attestee == &participant {
				warn!(target: LOG, "ignoring attestation for self: {:?}", attestee);
				continue;
			};
			if !meetup_participants.contains(attestee) {
				warn!(
					target: LOG,
					"ignoring attestation that isn't a meetup participant: {:?}", attestee
				);
				continue;
			};
			verified_attestees.insert(0, attestee.clone())
		}

		if verified_attestees.is_empty() {
			return Err(<Error<T>>::NoValidAttestations);
		}

		let count = <AttestationCount<T>>::get((cid, cindex));
		let mut idx = count.checked_add(1).ok_or(Error::<T>::CheckedMath)?;

		if <AttestationIndex<T>>::contains_key((cid, cindex), &participant) {
			// update previously registered set
			idx = <AttestationIndex<T>>::get((cid, cindex), &participant);
		} else {
			// add new set of attestees
			<AttestationCount<T>>::insert((cid, cindex), idx);
		}
		<AttestationRegistry<T>>::insert(
			(cid, cindex),
			idx,
			BoundedSlice::try_from(&verified_attestees[..])
				.map_err(|_| Error::<T>::TooManyAttestationsInBoundedVec)?,
		);
		<AttestationIndex<T>>::insert((cid, cindex), &participant, idx);
		let verified_count = verified_attestees.len() as u32;
		debug!(target: LOG, "successfully registered {} attestations", verified_count);
		Self::deposit_event(Event::AttestationsRegistered(
			*cid,
			meetup_index,
			verified_count,
			participant,
		));
		Ok(())
	}

	fn has_reputation(participant: &T::AccountId, cid: &CommunityIdentifier) -> bool {
		let reputation_lifetime = Self::reputation_lifetime();
		let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
		for i in 0..=reputation_lifetime {
			if Self::participant_reputation((*cid, cindex.saturating_sub(i)), participant)
				.is_verified()
			{
				return true;
			}
		}
		false
	}

	fn is_endorsed(
		participant: &T::AccountId,
		cc: &CommunityCeremony,
	) -> Option<CeremonyIndexType> {
		let reputation_lifetime = Self::reputation_lifetime();
		for i in 0..=reputation_lifetime {
			let cindex = cc.1.saturating_sub(i);
			if <Endorsees<T>>::contains_key((cc.0, cindex), participant) {
				return Some(cindex);
			}
		}
		None
	}

	pub fn validate_reputation(
		account_id: &T::AccountId,
		cid: &CommunityIdentifier,
		cindex: CeremonyIndexType,
	) -> bool {
		let current_cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
		if cindex < current_cindex.saturating_sub(Self::reputation_lifetime()) {
			return false;
		}
		<ParticipantReputation<T>>::get((*cid, cindex), account_id).is_verified()
	}

	#[cfg(any(test, feature = "runtime-benchmarks", feature = "mocks"))]
	// only to be used by tests
	pub fn fake_reputation(cidcindex: CommunityCeremony, account: &T::AccountId, rep: Reputation) {
		<ParticipantReputation<T>>::insert(cidcindex, account, rep);
		<ReputationCount<T>>::mutate(cidcindex, |b| *b += 1);
		<GlobalReputationCount<T>>::mutate(cidcindex.1, |b| *b += 1);
	}
}

impl<T: Config> OnCeremonyPhaseChange for Pallet<T> {
	fn on_ceremony_phase_change(new_phase: CeremonyPhaseType) {
		match new_phase {
			CeremonyPhaseType::Assigning => {
				let inactives = Self::update_inactivity_counters(
					<pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index()
						.saturating_sub(1),
					Self::inactivity_timeout(),
					<pallet_encointer_communities::Pallet<T>>::community_identifiers().to_vec(),
				);
				for inactive in inactives {
					Self::purge_community(inactive);
				}

				Self::generate_all_meetup_assignment_params();
			},
			CeremonyPhaseType::Attesting => {},
			CeremonyPhaseType::Registering => {
				let cindex = <pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
				// Clean up with a time delay, such that participants can claim their UBI in the following cycle.
				if cindex > Self::reputation_lifetime() {
					Self::purge_registry(
						cindex.saturating_sub(Self::reputation_lifetime()).saturating_sub(1),
					);
				}
			},
		}
	}
}

mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod migrations;
