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

use codec::{Decode, Encode};
use encointer_ceremonies_assignment::{
	assignment_fn_inverse, generate_assignment_function_params,
	math::{checked_ceil_division, find_prime_below, find_random_coprime_below},
	meetup_index, meetup_location, meetup_time,
};
use encointer_primitives::{
	balances::BalanceType,
	ceremonies::*,
	communities::{CommunityIdentifier, Location, NominalIncome},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
	RandomNumberGenerator,
};
use encointer_scheduler::OnCeremonyPhaseChange;
use frame_support::{
	dispatch::{DispatchResult, DispatchResultWithPostInfo},
	ensure,
	sp_std::cmp::min,
	traits::{Get, Randomness},
};
use frame_system::ensure_signed;
use log::{debug, error, info, trace, warn};
use scale_info::TypeInfo;
use sp_runtime::traits::{CheckedSub, IdentifyAccount, Member, Verify};
use sp_std::{cmp::max, prelude::*, vec};

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_timestamp::Config
		+ encointer_communities::Config
		+ encointer_balances::Config
		+ encointer_scheduler::Config
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type CeremonyMaster: EnsureOrigin<Self::Origin>;

		type Public: IdentifyAccount<AccountId = Self::AccountId>;
		type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;
		type RandomnessSource: Randomness<Self::Hash, Self::BlockNumber>;
		// Target number of participants per meetup
		#[pallet::constant]
		type MeetupSizeTarget: Get<u64>;
		// Minimum meetup size
		#[pallet::constant]
		type MeetupMinSize: Get<u64>;
		// Divisor used to determine the ratio of newbies allowed in relation to other participants
		#[pallet::constant]
		type MeetupNewbieLimitDivider: Get<u64>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn register_participant(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			proof: Option<ProofOfAttendance<T::Signature, T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				<encointer_scheduler::Pallet<T>>::current_phase() == CeremonyPhaseType::REGISTERING,
				Error::<T>::RegisteringPhaseRequired
			);

			ensure!(
				<encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();

			if Self::is_registered(cid, cindex, &sender) {
				return Err(<Error<T>>::ParticipantAlreadyRegistered.into())
			}

			if let Some(p) = &proof {
				// we accept proofs from other communities as well. no need to ensure cid
				ensure!(sender == p.prover_public, Error::<T>::WrongProofSubject);
				ensure!(p.ceremony_index < cindex, Error::<T>::ProofAcausal);
				ensure!(
					p.ceremony_index >=
						cindex.checked_sub(Self::reputation_lifetime()).unwrap_or(0),
					Error::<T>::ProofOutdated
				);
				ensure!(
					Self::participant_reputation(
						&(p.community_identifier, p.ceremony_index),
						&p.attendee_public
					) == Reputation::VerifiedUnlinked,
					Error::<T>::AttendanceUnverifiedOrAlreadyUsed
				);
				if Self::verify_attendee_signature(p.clone()).is_err() {
					return Err(<Error<T>>::BadProofOfAttendanceSignature.into())
				};

				// this reputation must now be burned so it can not be used again
				<ParticipantReputation<T>>::insert(
					&(p.community_identifier, p.ceremony_index),
					&p.attendee_public,
					Reputation::VerifiedLinked,
				);
				// register participant as reputable
				<ParticipantReputation<T>>::insert(
					(cid, cindex),
					&sender,
					Reputation::UnverifiedReputable,
				);
			};

			let participant_type = Self::register(cid, cindex, &sender, proof.is_some())?;

			debug!(target: LOG, "registered participant: {:?} as {:?}", sender, participant_type);
			Self::deposit_event(Event::ParticipantRegistered(cid, participant_type, sender));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn attest_claims(
			origin: OriginFor<T>,
			claims: Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				<encointer_scheduler::Pallet<T>>::current_phase() == CeremonyPhaseType::ATTESTING,
				Error::<T>::AttestationPhaseRequired
			);
			let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();
			ensure!(!claims.is_empty(), Error::<T>::NoValidClaims);
			let cid = claims[0].community_identifier; //safe; claims not empty checked above
			ensure!(
				<encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let meetup_index = Self::get_meetup_index((cid, cindex), &sender)
				.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;
			let mut meetup_participants =
				Self::get_meetup_participants((cid, cindex), meetup_index);
			ensure!(meetup_participants.contains(&sender), Error::<T>::OriginNotParticipant);
			meetup_participants.retain(|x| x != &sender);
			let num_registered = meetup_participants.len();
			ensure!(claims.len() <= num_registered, Error::<T>::TooManyClaims);
			let mut verified_attestees = vec![];

			let mlocation = Self::get_meetup_location((cid, cindex), meetup_index)
				.ok_or(<Error<T>>::MeetupLocationNotFound)?;

			let mtime =
				Self::get_meetup_time(mlocation).ok_or(<Error<T>>::MeetupTimeCalculationError)?;

			debug!(
				target: LOG,
				"{:?} attempts to register {:?} attested claims",
				sender,
				claims.len()
			);
			debug!(
				target: LOG,
				"meetup {} at location {:?} planned to happen at {:?} for cid {:?}",
				meetup_index,
				mlocation,
				mtime,
				cid
			);

			for claim in claims.iter() {
				let claimant = &claim.claimant_public;
				if claimant == &sender {
					warn!(target: LOG, "ignoring claim for self: {:?}", claimant);
					continue
				};
				if !meetup_participants.contains(claimant) {
					warn!(
						target: LOG,
						"ignoring claim that isn't a meetup participant: {:?}", claimant
					);
					continue
				};
				if claim.ceremony_index != cindex {
					warn!(
						target: LOG,
						"ignoring claim with wrong ceremony index: {}", claim.ceremony_index
					);
					continue
				};
				if claim.community_identifier != cid {
					warn!(
						target: LOG,
						"ignoring claim with wrong community identifier: {:?}",
						claim.community_identifier
					);
					continue
				};
				if claim.meetup_index != meetup_index {
					warn!(
						target: LOG,
						"ignoring claim with wrong meetup index: {}", claim.meetup_index
					);
					continue
				};
				if !<encointer_communities::Pallet<T>>::is_valid_location(&claim.location) {
					warn!(
						target: LOG,
						"ignoring claim with illegal geolocation: {:?}", claim.location
					);
					continue
				};
				if <encointer_communities::Pallet<T>>::haversine_distance(
					&mlocation,
					&claim.location,
				) > Self::location_tolerance()
				{
					warn!(
						target: LOG,
						"ignoring claim beyond location tolerance: {:?}", claim.location
					);
					continue
				};
				if let Some(dt) = mtime.checked_sub(&claim.timestamp) {
					if dt > Self::time_tolerance() {
						warn!(
							target: LOG,
							"ignoring claim beyond time tolerance (too early): {:?}",
							claim.timestamp
						);
						continue
					};
				} else if let Some(dt) = claim.timestamp.checked_sub(&mtime) {
					if dt > Self::time_tolerance() {
						warn!(
							target: LOG,
							"ignoring claim beyond time tolerance (too late): {:?}",
							claim.timestamp
						);
						continue
					};
				}
				if !claim.verify_signature() {
					warn!(target: LOG, "ignoring claim with bad signature for {:?}", claimant);
					continue
				};
				// claim is legit. insert it!
				verified_attestees.insert(0, claimant.clone());

				// is it a problem if this number isn't equal for all claims? Guess not.
				// is it a problem that this gets inserted multiple times? Guess not.
				<MeetupParticipantCountVote<T>>::insert(
					(cid, cindex),
					&claimant,
					&claim.number_of_participants_confirmed,
				);
			}
			if verified_attestees.is_empty() {
				return Err(<Error<T>>::NoValidClaims.into())
			}

			let count = <AttestationCount<T>>::get((cid, cindex));
			let mut idx = count.checked_add(1).ok_or(Error::<T>::CheckedMath)?;

			if <AttestationIndex<T>>::contains_key((cid, cindex), &sender) {
				// update previously registered set
				idx = <AttestationIndex<T>>::get((cid, cindex), &sender);
			} else {
				// add new set of attestees
				let new_count = count
					.checked_add(1)
					.ok_or("[EncointerCeremonies]: Overflow adding set of attestees to registry")?;
				<AttestationCount<T>>::insert((cid, cindex), new_count);
			}
			<AttestationRegistry<T>>::insert((cid, cindex), &idx, &verified_attestees);
			<AttestationIndex<T>>::insert((cid, cindex), &sender, &idx);
			let verified_count = verified_attestees.len() as u32;
			debug!(target: LOG, "successfully registered {} claims", verified_count);
			Self::deposit_event(Event::AttestationsRegistered(
				cid,
				meetup_index,
				verified_count,
				sender,
			));
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn endorse_newcomer(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			newbie: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			ensure!(
				<encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			ensure!(
				<encointer_communities::Pallet<T>>::bootstrappers(&cid).contains(&sender),
				Error::<T>::AuthorizationRequired
			);

			ensure!(
				<BurnedBootstrapperNewbieTickets<T>>::get(&cid, &sender) <
					Self::endorsement_tickets_per_bootstrapper(),
				Error::<T>::NoMoreNewbieTickets
			);

			let mut cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();
			if <encointer_scheduler::Pallet<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
				cindex += 1; //safe; cindex comes from within, will not overflow at +1/d
			}
			ensure!(
				!<Endorsees<T>>::contains_key((cid, cindex), &newbie),
				Error::<T>::AlreadyEndorsed
			);

			<BurnedBootstrapperNewbieTickets<T>>::mutate(&cid, sender.clone(), |b| *b += 1); // safe; limited by AMOUNT_NEWBIE_TICKETS
			<Endorsees<T>>::insert((cid, cindex), newbie.clone(), ());
			<EndorseesCount<T>>::mutate((cid, cindex), |c| *c += 1); // safe; limited by AMOUNT_NEWBIE_TICKETS

			debug!(target: LOG, "bootstrapper {:?} endorsed newbie: {:?}", sender, newbie);
			Self::deposit_event(Event::EndorsedParticipant(cid, sender, newbie));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			let (meetup_index, reward_count) =
				Self::validate_one_meetup_and_issue_rewards(&sender, &cid)?;

			Self::deposit_event(Event::RewardsIssued(cid, meetup_index, reward_count));
			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_inactivity_timeout(
			origin: OriginFor<T>,
			inactivity_timeout: InactivityTimeoutType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<InactivityTimeout<T>>::put(inactivity_timeout);
			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_endorsement_tickets_per_bootstrapper(
			origin: OriginFor<T>,
			endorsement_tickets_per_bootstrapper: EndorsementTicketsPerBootstrapperType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<EndorsementTicketsPerBootstrapper<T>>::put(endorsement_tickets_per_bootstrapper);
			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_reputation_lifetime(
			origin: OriginFor<T>,
			reputation_lifetime: ReputationLifetimeType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			<ReputationLifetime<T>>::put(reputation_lifetime);
			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_meetup_time_offset(
			origin: OriginFor<T>,
			meetup_time_offset: MeetupTimeOffsetType,
		) -> DispatchResultWithPostInfo {
			<T as pallet::Config>::CeremonyMaster::ensure_origin(origin)?;
			if <encointer_scheduler::Pallet<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
				return Err(<Error<T>>::WrongPhaseForChangingMeetupTimeOffset.into())
			}
			<MeetupTimeOffset<T>>::put(meetup_time_offset);
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
		RewardsIssued(CommunityIdentifier, MeetupIndexType, u8),
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
		NoValidClaims,
		/// sender doesn't have the necessary authority to perform action
		AuthorizationRequired,
		/// the action can only be performed during REGISTERING phase
		RegisteringPhaseRequired,
		/// the action can only be performed during ATTESTING phase
		AttestationPhaseRequired,
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
		/// origin not part of this meetup
		OriginNotParticipant,
		/// can't have more claims than other meetup participants
		TooManyClaims,
		/// bootstrapper has run out of newbie tickets
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
	}

	#[pallet::storage]
	#[pallet::getter(fn bootstrapper_newbie_tickets)]
	pub(super) type BurnedBootstrapperNewbieTickets<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Blake2_128Concat,
		T::AccountId,
		u8,
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
		StorageMap<_, Blake2_128Concat, CommunityCeremony, u64, ValueQuery>;

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
		Vec<T::AccountId>,
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
		(),
		ValueQuery,
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

	/// The number newbies a bootstrapper can endorse to accelerate community growth
	#[pallet::storage]
	#[pallet::getter(fn endorsement_tickets_per_bootstrapper)]
	pub(super) type EndorsementTicketsPerBootstrapper<T: Config> =
		StorageValue<_, EndorsementTicketsPerBootstrapperType, ValueQuery>;

	/// The number of ceremony cycles that a participant's reputation is valid for
	#[pallet::storage]
	#[pallet::getter(fn reputation_lifetime)]
	pub(super) type ReputationLifetime<T: Config> =
		StorageValue<_, ReputationLifetimeType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn meetup_time_offset)]
	pub(super) type MeetupTimeOffset<T: Config> = StorageValue<_, MeetupTimeOffsetType, ValueQuery>;

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
		pub endorsement_tickets_per_bootstrapper: EndorsementTicketsPerBootstrapperType,
		pub reputation_lifetime: ReputationLifetimeType,
		pub meetup_time_offset: MeetupTimeOffsetType,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		fn default() -> Self {
			Self {
				ceremony_reward: Default::default(),
				location_tolerance: Default::default(),
				time_tolerance: Default::default(),
				inactivity_timeout: Default::default(),
				endorsement_tickets_per_bootstrapper: Default::default(),
				reputation_lifetime: Default::default(),
				meetup_time_offset: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		fn build(&self) {
			<CeremonyReward<T>>::put(&self.ceremony_reward);
			<LocationTolerance<T>>::put(&self.location_tolerance);
			<TimeTolerance<T>>::put(&self.time_tolerance);
			<InactivityTimeout<T>>::put(&self.inactivity_timeout);
			<EndorsementTicketsPerBootstrapper<T>>::put(&self.endorsement_tickets_per_bootstrapper);
			<ReputationLifetime<T>>::put(&self.reputation_lifetime);
			<MeetupTimeOffset<T>>::put(&self.meetup_time_offset);
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_reputations(
		account: &T::AccountId,
	) -> Vec<(CeremonyIndexType, CommunityReputation)> {
		return ParticipantReputation::<T>::iter()
			.filter(|t| &t.1 == account)
			.map(|t| (t.0 .1, CommunityReputation::new(t.0 .0, t.2)))
			.collect()
	}

	fn register(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
		is_reputable: bool,
	) -> Result<ParticipantType, Error<T>> {
		let participant_type =
			if <encointer_communities::Pallet<T>>::bootstrappers(cid).contains(&sender) {
				let participant_index = <BootstrapperCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<BootstrapperRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
				<BootstrapperIndex<T>>::insert((cid, cindex), &sender, &participant_index);
				<BootstrapperCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Bootstrapper
			} else if !(<encointer_balances::Pallet<T>>::total_issuance(cid) > 0) {
				return Err(Error::<T>::OnlyBootstrappers)
			} else if is_reputable {
				let participant_index = <ReputableCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<ReputableRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
				<ReputableIndex<T>>::insert((cid, cindex), &sender, &participant_index);
				<ReputableCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Reputable
			} else if <Endorsees<T>>::contains_key((cid, cindex), &sender) {
				let participant_index = <EndorseeCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<EndorseeRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
				<EndorseeIndex<T>>::insert((cid, cindex), &sender, &participant_index);
				<EndorseeCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Endorsee
			} else {
				let participant_index = <NewbieCount<T>>::get((cid, cindex))
					.checked_add(1)
					.ok_or(Error::<T>::RegistryOverflow)?;
				<NewbieRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
				<NewbieIndex<T>>::insert((cid, cindex), &sender, &participant_index);
				<NewbieCount<T>>::insert((cid, cindex), participant_index);
				ParticipantType::Newbie
			};
		Ok(participant_type)
	}

	fn is_registered(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
	) -> bool {
		<BootstrapperIndex<T>>::contains_key((cid, cindex), &sender) ||
			<ReputableIndex<T>>::contains_key((cid, cindex), &sender) ||
			<EndorseeIndex<T>>::contains_key((cid, cindex), &sender) ||
			<NewbieIndex<T>>::contains_key((cid, cindex), &sender)
	}

	fn purge_community_ceremony(cc: CommunityCeremony) {
		<BootstrapperRegistry<T>>::remove_prefix(cc, None);
		<BootstrapperIndex<T>>::remove_prefix(cc, None);
		<BootstrapperCount<T>>::insert(cc, 0);

		<ReputableRegistry<T>>::remove_prefix(cc, None);
		<ReputableIndex<T>>::remove_prefix(cc, None);
		<ReputableCount<T>>::insert(cc, 0);

		<EndorseeRegistry<T>>::remove_prefix(cc, None);
		<EndorseeIndex<T>>::remove_prefix(cc, None);
		<EndorseeCount<T>>::insert(cc, 0);

		<NewbieRegistry<T>>::remove_prefix(cc, None);
		<NewbieIndex<T>>::remove_prefix(cc, None);
		<NewbieCount<T>>::insert(cc, 0);

		<AssignmentCounts<T>>::insert(cc, AssignmentCount::default());

		Assignments::<T>::remove(cc);

		<Endorsees<T>>::remove_prefix(cc, None);
		<MeetupCount<T>>::insert(cc, 0);
		<AttestationRegistry<T>>::remove_prefix(cc, None);
		<AttestationIndex<T>>::remove_prefix(cc, None);
		<AttestationCount<T>>::insert(cc, 0);
		<MeetupParticipantCountVote<T>>::remove_prefix(cc, None);

		<IssuedRewards<T>>::remove_prefix(cc, None);
	}

	fn purge_registry(cindex: CeremonyIndexType) {
		let cids = <encointer_communities::Pallet<T>>::community_identifiers();
		for cid in cids.into_iter() {
			Self::purge_community_ceremony((cid, cindex));
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
			return Ok(())
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
			<encointer_communities::Pallet<T>>::get_locations(&community_ceremony.0).len() as u64;

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
			<encointer_communities::Pallet<T>>::get_locations(&community_ceremony.0).len() as u64;
		debug!(
			target: LOG,
			"Number of locations for cid {:?} is {:?}", community_ceremony.0, num_locations
		);
		if num_locations == 0 {
			return Err(<Error<T>>::NoLocationsAvailable.into())
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
			max_num_meetups.checked_mul(meetup_multiplier).ok_or(Error::<T>::CheckedMath)? -
				num_registered_bootstrappers;

		let num_assigned_reputables = min(num_registered_reputables, seats_left);
		seats_left -= num_assigned_reputables; //safe; given by minimum above

		let num_assigned_endorsees = min(num_registered_endorsees, seats_left);
		seats_left -= num_assigned_endorsees; //safe; given by minimum above

		let num_assigned_newbies = min(
			min(num_registered_newbies, seats_left),
			(num_registered_bootstrappers + num_assigned_reputables + num_assigned_endorsees) /
				T::MeetupNewbieLimitDivider::get(), //safe; sum equals total
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

	fn get_inactive_communities(
		cindex: u32,
		inactivity_timeout: u32,
		cids: Vec<CommunityIdentifier>,
	) -> Vec<CommunityIdentifier> {
		let mut inactives = vec![];
		for cid in cids {
			if <IssuedRewards<T>>::iter_prefix_values((cid, cindex)).next().is_some() {
				<InactivityCounters<T>>::insert(cid, 0);
			} else {
				let current = Self::inactivity_counters(cid).unwrap_or(0);
				if current >= inactivity_timeout {
					inactives.push(cid.clone());
				} else {
					<InactivityCounters<T>>::insert(cid, current + 1);
				}
			}
		}
		return inactives
	}

	fn purge_community(cid: CommunityIdentifier) {
		let current = <encointer_scheduler::Pallet<T>>::current_ceremony_index();
		let reputation_lifetime = Self::reputation_lifetime();
		for cindex in max(current - reputation_lifetime, 0)..current {
			if cindex > reputation_lifetime {
				Self::purge_registry(cindex - reputation_lifetime - 1);
			}
		}
		<encointer_communities::Pallet<T>>::remove_community(cid);
	}

	fn generate_all_meetup_assignment_params() {
		let cids = <encointer_communities::Pallet<T>>::community_identifiers();
		let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();

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

	fn get_meetup_index(
		community_ceremony: CommunityCeremony,
		participant: &T::AccountId,
	) -> Option<MeetupIndexType> {
		let meetup_count = Self::meetup_count(community_ceremony);
		let assignment_count = Self::assignment_counts(community_ceremony);

		let assignment = Self::assignments(community_ceremony);

		if <BootstrapperIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::bootstrapper_index(community_ceremony, &participant) - 1;
			if participant_index < assignment_count.bootstrappers {
				return meetup_index(
					participant_index,
					assignment.bootstrappers_reputables,
					meetup_count,
				)
			}
		}
		if <ReputableIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::reputable_index(community_ceremony, &participant) - 1;
			if participant_index < assignment_count.reputables {
				return meetup_index(
					participant_index + assignment_count.bootstrappers,
					assignment.bootstrappers_reputables,
					meetup_count,
				)
			}
		}

		if <EndorseeIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::endorsee_index(community_ceremony, &participant) - 1;
			if participant_index < assignment_count.endorsees {
				return meetup_index(participant_index, assignment.endorsees, meetup_count)
			}
		}

		if <NewbieIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::newbie_index(community_ceremony, &participant) - 1;
			if participant_index < assignment_count.newbies {
				return meetup_index(participant_index, assignment.newbies, meetup_count)
			}
		}

		None
	}

	fn get_meetup_participants(
		community_ceremony: CommunityCeremony,
		mut meetup_index: MeetupIndexType,
	) -> Vec<T::AccountId> {
		let mut result: Vec<T::AccountId> = vec![];
		let meetup_count = Self::meetup_count(community_ceremony);

		//safe; meetup index conversion from 1 based to 0 based
		meetup_index -= 1;
		if meetup_index > meetup_count {
			error!(
				target: LOG,
				"Invalid meetup index > meetup count: {}, {}", meetup_index, meetup_count
			);
			return vec![]
		}

		let params = Self::assignments(community_ceremony);

		let assigned = Self::assignment_counts(community_ceremony);

		let bootstrappers_reputables = assignment_fn_inverse(
			meetup_index,
			params.bootstrappers_reputables,
			meetup_count,
			assigned.bootstrappers + assigned.reputables,
		);
		for p in bootstrappers_reputables {
			if p < assigned.bootstrappers {
				//safe; small number per meetup
				match Self::bootstrapper_registry(community_ceremony, &(p + 1)) {
					Some(bs) => result.push(bs),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Bootstrapper not found!!"
					),
				}
			} else if p < assigned.bootstrappers + assigned.reputables {
				//safe; small number per meetup
				match Self::reputable_registry(
					community_ceremony,
					&(p - assigned.bootstrappers + 1),
				) {
					Some(r) => result.push(r),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Reputable not found!!"
					),
				};
			}
		}

		let endorsees =
			assignment_fn_inverse(meetup_index, params.endorsees, meetup_count, assigned.endorsees);
		for p in endorsees {
			if p < assigned.endorsees {
				//safe; small number per meetup
				match Self::endorsee_registry(community_ceremony, &(p + 1)) {
					Some(e) => result.push(e),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Endorsee not found!!"
					),
				};
			}
		}

		let newbies =
			assignment_fn_inverse(meetup_index, params.newbies, meetup_count, assigned.newbies);
		for p in newbies {
			if p < assigned.newbies {
				//safe; small number per meetup
				match Self::newbie_registry(community_ceremony, &(p + 1)) {
					Some(n) => result.push(n),
					None => error!(
						target: LOG,
						"[Ceremonies::get_meetup_participants] Newbie not found!!"
					),
				};
			}
		}

		return result
	}

	fn verify_attendee_signature(
		proof: ProofOfAttendance<T::Signature, T::AccountId>,
	) -> DispatchResult {
		match proof.attendee_signature.verify(
			&(proof.prover_public, proof.ceremony_index).encode()[..],
			&proof.attendee_public,
		) {
			true => Ok(()),
			false => Err(<Error<T>>::BadAttendeeSignature.into()),
		}
	}

	fn validate_one_meetup_and_issue_rewards(
		participant: &T::AccountId,
		cid: &CommunityIdentifier,
	) -> Result<(MeetupIndexType, u8), Error<T>> {
		if <encointer_scheduler::Pallet<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
			return Err(<Error<T>>::WrongPhaseForClaimingRewards.into())
		}

		let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index() - 1; //safe; cindex comes from within
		let reward = Self::nominal_income(cid);
		let meetup_index = Self::get_meetup_index((*cid, cindex), participant)
			.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;

		if <IssuedRewards<T>>::contains_key((cid, cindex), meetup_index) {
			return Err(<Error<T>>::RewardsAlreadyIssued.into())
		}
		info!(
			target: LOG,
			"validating meetup {:?} for cid {:?} triggered by {:?}", meetup_index, cid, participant
		);
		// first, evaluate votes on how many participants showed up
		let (n_confirmed, vote_count) = match Self::ballot_meetup_n_votes(cid, cindex, meetup_index)
		{
			Some(nn) => nn,
			_ => return Err(<Error<T>>::VotesNotDependable.into()),
		};
		debug!(
			target: LOG,
			"  ballot confirms {:?} participants with {:?} votes", n_confirmed, vote_count
		);
		let meetup_participants = Self::get_meetup_participants((*cid, cindex), meetup_index);
		let mut reward_count = 0;
		for participant in &meetup_participants {
			if Self::meetup_participant_count_vote((cid, cindex), &participant) != n_confirmed {
				debug!(
					target: LOG,
					"skipped participant because of wrong participant count vote: {:?}",
					participant
				);
				continue
			}

			match Self::attestation_registry(
				(cid, cindex),
				&Self::attestation_index((*cid, cindex), &participant),
			) {
				Some(attestees) =>
					if attestees.len() < (vote_count - 1) as usize {
						debug!(
							target: LOG,
							"skipped participant because didn't testify for honest peers: {:?}",
							participant
						);
						continue
					},
				None => continue,
			};

			let mut was_attested_count = 0u32;
			for other_participant in &meetup_participants {
				if other_participant == participant {
					continue
				}
				if let Some(attestees_from_other) = Self::attestation_registry(
					(cid, cindex),
					&Self::attestation_index((cid, cindex), &other_participant),
				) {
					if attestees_from_other.contains(&participant) {
						was_attested_count += 1; // <= number of meetup participants
					}
				}
			}

			if was_attested_count < (vote_count - 1) {
				debug!(
					"skipped participant because of too few attestations ({}): {:?}",
					was_attested_count, participant
				);
				continue
			}

			trace!(target: LOG, "participant merits reward: {:?}", participant);
			if <encointer_balances::Pallet<T>>::issue(*cid, &participant, reward).is_ok() {
				<ParticipantReputation<T>>::insert(
					(cid, cindex),
					&participant,
					Reputation::VerifiedUnlinked,
				);
			}
			reward_count += 1;
		}
		<IssuedRewards<T>>::insert((cid, cindex), meetup_index, ());
		info!(target: LOG, "issuing rewards completed");
		Ok((meetup_index, reward_count))
	}

	/// count all votes for a meetup
	/// returns an option for (N, n) where N is the confirmed number of participants and
	/// n is the number of votes confirming N
	fn ballot_meetup_n_votes(
		cid: &CommunityIdentifier,
		cindex: CeremonyIndexType,
		meetup_idx: MeetupIndexType,
	) -> Option<(u32, u32)> {
		let meetup_participants = Self::get_meetup_participants((*cid, cindex), meetup_idx);
		// first element is n, second the count of votes for n
		let mut n_vote_candidates: Vec<(u32, u32)> = vec![];
		for p in meetup_participants {
			let this_vote = match Self::meetup_participant_count_vote((cid, cindex), &p) {
				n if n > 0 => n,
				_ => continue,
			};
			match n_vote_candidates.iter().position(|&(n, _c)| n == this_vote) {
				Some(idx) => n_vote_candidates[idx].1 += 1, //safe; <= number of candidates
				_ => n_vote_candidates.insert(0, (this_vote, 1)),
			};
		}
		if n_vote_candidates.is_empty() {
			debug!(target: LOG, "ballot empty for meetup {:?}, cid: {:?}", meetup_idx, cid);
			return None
		}
		// sort by descending vote count
		n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
		if n_vote_candidates[0].1 < 3 {
			//safe; n_vote_candidate not empty checked above
			debug!(
				target: LOG,
				"ballot doesn't reach dependable majority for meetup {:?}, cid: {:?}",
				meetup_idx,
				cid
			);
			return None
		}
		Some(n_vote_candidates[0])
	}

	pub fn get_meetup_location(
		cc: CommunityCeremony,
		meetup_idx: MeetupIndexType,
	) -> Option<Location> {
		let locations = <encointer_communities::Pallet<T>>::get_locations(&cc.0);
		let assignment_params = Self::assignments(cc).locations;

		meetup_location(meetup_idx, locations, assignment_params)
	}

	// this function only works during ATTESTING, so we're keeping it for private use
	fn get_meetup_time(location: Location) -> Option<T::Moment> {
		if !(<encointer_scheduler::Pallet<T>>::current_phase() == CeremonyPhaseType::ATTESTING) {
			return None
		}

		let duration =
			<encointer_scheduler::Pallet<T>>::phase_durations(CeremonyPhaseType::ATTESTING);
		let next = <encointer_scheduler::Pallet<T>>::next_phase_timestamp();
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
	fn nominal_income(cid: &CommunityIdentifier) -> NominalIncome {
		encointer_communities::NominalIncome::<T>::try_get(cid)
			.unwrap_or_else(|_| Self::ceremony_reward())
	}

	#[cfg(test)]
	// only to be used by tests
	fn fake_reputation(cidcindex: CommunityCeremony, account: &T::AccountId, rep: Reputation) {
		<ParticipantReputation<T>>::insert(&cidcindex, account, rep);
	}
}

impl<T: Config> OnCeremonyPhaseChange for Pallet<T> {
	fn on_ceremony_phase_change(new_phase: CeremonyPhaseType) {
		match new_phase {
			CeremonyPhaseType::ASSIGNING => {
				Self::generate_all_meetup_assignment_params();
			},
			CeremonyPhaseType::ATTESTING => {},
			CeremonyPhaseType::REGISTERING => {
				let cindex = <encointer_scheduler::Pallet<T>>::current_ceremony_index();
				// Clean up with a time delay, such that participants can claim their UBI in the following cycle.
				if cindex > Self::reputation_lifetime() {
					Self::purge_registry(cindex - Self::reputation_lifetime() - 1);
				}
				let inactives = Self::get_inactive_communities(
					<encointer_scheduler::Pallet<T>>::current_ceremony_index() - 1,
					Self::inactivity_timeout(),
					<encointer_communities::Pallet<T>>::community_identifiers(),
				);
				for inactive in inactives {
					Self::purge_community(inactive);
				}
			},
		}
	}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
