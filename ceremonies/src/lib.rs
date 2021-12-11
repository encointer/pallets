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

use crate::math::{checked_ceil_division, find_prime_below, find_random_coprime_below};
use codec::{Decode, Encode};
use encointer_primitives::{
	balances::BalanceType,
	ceremonies::{
		consts::{AMOUNT_NEWBIE_TICKETS, REPUTATION_LIFETIME},
		*,
	},
	communities::{CommunityIdentifier, Degree, Location, LossyFrom, NominalIncome},
	scheduler::{CeremonyIndexType, CeremonyPhaseType},
	RandomNumberGenerator,
};
use encointer_scheduler::OnCeremonyPhaseChange;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	ensure,
	sp_std::cmp::min,
	storage::{StorageDoubleMap, StorageMap},
	traits::{Get, Randomness},
};
use frame_system::ensure_signed;
use log::{debug, error, info, trace, warn};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{CheckedSub, IdentifyAccount, Member, Verify},
	SaturatedConversion,
};
use sp_std::{prelude::*, vec};

// Logger target
const LOG: &str = "encointer";

pub trait Config:
	frame_system::Config
	+ pallet_timestamp::Config
	+ encointer_communities::Config
	+ encointer_balances::Config
	+ encointer_scheduler::Config
{
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type Public: IdentifyAccount<AccountId = Self::AccountId>;
	type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;
	type RandomnessSource: Randomness<Self::Hash, Self::BlockNumber>;
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Config> as EncointerCeremonies {
		BurnedBootstrapperNewbieTickets get(fn bootstrapper_newbie_tickets): double_map hasher(blake2_128_concat) CommunityIdentifier, hasher(blake2_128_concat) T::AccountId => u8;

		// everyone who registered for a ceremony
		// caution: index starts with 1, not 0! (because null and 0 is the same for state storage)
		BootstrapperRegistry get(fn bootstrapper_registry): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) ParticipantIndexType => T::AccountId;
		BootstrapperIndex get(fn bootstrapper_index): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => ParticipantIndexType;
		BootstrapperCount get(fn bootstrapper_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;

		ReputableRegistry get(fn reputable_registry): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) ParticipantIndexType => T::AccountId;
		ReputableIndex get(fn reputable_index): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => ParticipantIndexType;
		ReputableCount get(fn reputable_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;

		EndorseeRegistry get(fn endorsee_registry): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) ParticipantIndexType => T::AccountId;
		EndorseeIndex get(fn endorsee_index): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => ParticipantIndexType;
		EndorseeCount get(fn endorsee_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;

		NewbieRegistry get(fn newbie_registry): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) ParticipantIndexType => T::AccountId;
		NewbieIndex get(fn newbie_index): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => ParticipantIndexType;
		NewbieCount get(fn newbie_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;

		AssignmentCounts get(fn assignment_counts): map hasher(blake2_128_concat) CommunityCeremony => AssignmentCount;

		Assignments get(fn assignments): map hasher(blake2_128_concat) CommunityCeremony => Assignment;

		ParticipantReputation get(fn participant_reputation): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => Reputation;
		// newbies granted a ticket from a bootstrapper for the next ceremony. See https://substrate.dev/recipes/map-set.html for the rationale behind the double_map approach.
		Endorsees get(fn endorsees): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => ();
		EndorseesCount get(fn endorsees_count): map hasher(blake2_128_concat) CommunityCeremony => u64;


		MeetupCount get(fn meetup_count): map hasher(blake2_128_concat) CommunityCeremony => MeetupIndexType;

		// collect fellow meetup participants accounts who attested key account
		// caution: index starts with 1, not 0! (because null and 0 is the same for state storage)
		AttestationRegistry get(fn attestation_registry): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) AttestationIndexType => Vec<T::AccountId>;
		AttestationIndex get(fn attestation_index): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => AttestationIndexType;
		AttestationCount get(fn attestation_count): map hasher(blake2_128_concat) CommunityCeremony => AttestationIndexType;
		// how many peers does each participants observe at their meetup
		MeetupParticipantCountVote get(fn meetup_participant_count_vote): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) T::AccountId => u32;
		/// the default UBI for a ceremony attendee if no community specific value is set.
		CeremonyReward get(fn ceremony_reward) config(): BalanceType;
		// [m] distance from assigned meetup location
		LocationTolerance get(fn location_tolerance) config(): u32;
		// [ms] time tolerance for meetup moment
		TimeTolerance get(fn time_tolerance) config(): T::Moment;

		IssuedRewards get(fn issued_rewards): double_map hasher(blake2_128_concat) CommunityCeremony, hasher(blake2_128_concat) MeetupIndexType => ();
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		fn deposit_event() = default;
		type Error = Error<T>;

		#[weight = 10_000]
		pub fn grant_reputation(origin, cid: CommunityIdentifier, reputable: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(sender == <encointer_scheduler::Module<T>>::ceremony_master(), Error::<T>::AuthorizationRequired);
			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
			<ParticipantReputation<T>>::insert(&(cid, cindex-1), reputable, Reputation::VerifiedUnlinked);
			info!(target: LOG, "granting reputation to {:?}", sender);
			Ok(())
		}

		#[weight = 10_000]
		pub fn register_participant(origin, cid: CommunityIdentifier, proof: Option<ProofOfAttendance<T::Signature, T::AccountId>>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::REGISTERING,
				Error::<T>::RegisteringPhaseRequired);

			ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity);

			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();

			if Self::is_registered(cid, cindex, &sender) {
				return Err(<Error<T>>::ParticipantAlreadyRegistered.into());
			}

			if let Some(p) = &proof {
				// we accept proofs from other communities as well. no need to ensure cid
				ensure!(sender == p.prover_public, Error::<T>::WrongProofSubject);
				ensure!(p.ceremony_index < cindex, Error::<T>::ProofAcausal);
				ensure!(p.ceremony_index >= cindex-REPUTATION_LIFETIME, Error::<T>::ProofOutdated);
				ensure!(Self::participant_reputation(&(p.community_identifier, p.ceremony_index),
					&p.attendee_public) == Reputation::VerifiedUnlinked,
					Error::<T>::AttendanceUnverifiedOrAlreadyUsed);
				if Self::verify_attendee_signature(p.clone()).is_err() {
					return Err(<Error<T>>::BadProofOfAttendanceSignature.into());
				};

				// this reputation must now be burned so it can not be used again
				<ParticipantReputation<T>>::insert(&(p.community_identifier, p.ceremony_index),
					&p.attendee_public, Reputation::VerifiedLinked);
				// register participant as reputable
				<ParticipantReputation<T>>::insert((cid, cindex),
					&sender, Reputation::UnverifiedReputable);
			};

			Self::register(cid, cindex, &sender, proof.is_some())?;

			debug!(target: LOG, "registered participant: {:?}", sender);
			Ok(())
		}

		#[weight = 10_000]
		pub fn attest_claims(origin, claims: Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::ATTESTING,
				Error::<T>::AttestationPhaseRequired);
			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
			ensure!(!claims.is_empty(), Error::<T>::NoValidClaims);
			let cid = claims[0].community_identifier;
			ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity);

			let meetup_index = Self::get_meetup_index((cid, cindex), &sender)?;
			let mut meetup_participants = Self::get_meetup_participants((cid, cindex), meetup_index);
			ensure!(meetup_participants.contains(&sender), Error::<T>::OriginNotParticipant);
			meetup_participants.retain(|x| x != &sender);
			let num_registered = meetup_participants.len();
			ensure!(claims.len() <= num_registered, Error::<T>::TooManyClaims);
			let mut verified_attestees = vec!();

			let mlocation = if let Some(l) = Self::get_meetup_location((cid, cindex), meetup_index)
				{ l } else { return Err(<Error<T>>::MeetupLocationNotFound.into()) };
			let mtime = if let Some(t) = Self::get_meetup_time((cid, cindex), meetup_index)
				{ t } else { return Err(<Error<T>>::MeetupTimeCalculationError.into()) };
			debug!(target: LOG, "meetup {} at location {:?} should happen at {:?} for cid {:?}",
				meetup_index, mlocation, mtime, cid);
			for claim in claims.iter() {
				let claimant = &claim.claimant_public;
				if claimant == &sender {
					warn!(target: LOG,
						"ignoring claim that is from sender: {:?}",
						claimant);
					continue };
				if !meetup_participants.contains(claimant) {
					warn!(target: LOG,
						"ignoring claim that isn't a meetup participant: {:?}",
						claimant);
					continue };
				if claim.ceremony_index != cindex {
					warn!(target: LOG,
						"ignoring claim with wrong ceremony index: {}",
						claim.ceremony_index);
					continue };
				if claim.community_identifier != cid {
					warn!(target: LOG,
						"ignoring claim with wrong community identifier: {:?}",
						claim.community_identifier);
					continue };
				if claim.meetup_index != meetup_index {
					warn!(target: LOG,
						"ignoring claim with wrong meetup index: {}",
						claim.meetup_index);
					continue };
				if !<encointer_communities::Module<T>>::is_valid_location(
					&claim.location) {
						warn!(target: LOG,
							"ignoring claim with illegal geolocation: {:?}",
							claim.location);
						continue };
				if <encointer_communities::Module<T>>::haversine_distance(
					&mlocation, &claim.location) > Self::location_tolerance() {
						warn!(target: LOG,
							"ignoring claim beyond location tolerance: {:?}",
							claim.location);
						continue };
				if let Some(dt) = mtime.checked_sub(&claim.timestamp) {
					if dt > Self::time_tolerance() {
						warn!(target: LOG,
							"ignoring claim beyond time tolerance (too early): {:?}",
							claim.timestamp);
						continue };
				} else if let Some(dt) = claim.timestamp.checked_sub(&mtime) {
					if dt > Self::time_tolerance() {
						warn!(target: LOG,
							"ignoring claim beyond time tolerance (too late): {:?}",
							claim.timestamp);
						continue };
				}
				if !claim.verify_signature() {
					warn!(target: LOG, "ignoring claim with bad signature for {:?}", claimant);
					continue };
				// claim is legit. insert it!
				verified_attestees.insert(0, claimant.clone());

				// is it a problem if this number isn't equal for all claims? Guess not.
				// is it a problem that this gets inserted multiple times? Guess not.
				<MeetupParticipantCountVote<T>>::insert((cid, cindex), &claimant, &claim.number_of_participants_confirmed);
			}
			if verified_attestees.is_empty() {
				return Err(<Error<T>>::NoValidClaims.into());
			}

			let count = <AttestationCount>::get((cid, cindex));
			let mut idx = count+1;

			if <AttestationIndex<T>>::contains_key((cid, cindex), &sender) {
				// update previously registered set
				idx = <AttestationIndex<T>>::get((cid, cindex), &sender);
			} else {
				// add new set of attestees
				let new_count = count.checked_add(1).
					ok_or("[EncointerCeremonies]: Overflow adding set of attestees to registry")?;
				<AttestationCount>::insert((cid, cindex), new_count);
			}
			<AttestationRegistry<T>>::insert((cid, cindex), &idx, &verified_attestees);
			<AttestationIndex<T>>::insert((cid, cindex), &sender, &idx);
			debug!(target: LOG,
				"successfully registered {} claims", verified_attestees.len());
			Ok(())
		}

		#[weight = 10_000]
		pub fn endorse_newcomer(origin, cid: CommunityIdentifier, newbie: T::AccountId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(<encointer_communities::Module<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity);

			ensure!(<encointer_communities::Module<T>>::bootstrappers(&cid).contains(&sender),
			Error::<T>::AuthorizationRequired);

			ensure!(<BurnedBootstrapperNewbieTickets<T>>::get(&cid, &sender) < AMOUNT_NEWBIE_TICKETS,
			Error::<T>::NoMoreNewbieTickets);

			let mut cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
			if <encointer_scheduler::Module<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
				cindex += 1;
			}
			ensure!(!<Endorsees<T>>::contains_key((cid, cindex), &newbie),
			Error::<T>::AlreadyEndorsed);

			<BurnedBootstrapperNewbieTickets<T>>::mutate(&cid, sender,|b| *b += 1);
			debug!(target: LOG, "endorsed newbie: {:?}", newbie);
			<Endorsees<T>>::insert((cid, cindex), newbie, ());
			<EndorseesCount>::mutate((cid, cindex), |c| *c += 1);
			Ok(())
		}

		#[weight = 10_000]
		pub fn claim_rewards(origin, cid: CommunityIdentifier) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			Self::issue_rewards(&sender, &cid)
		}
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
	{
		ParticipantRegistered(AccountId),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
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
		/// Trying to issue rewards for a meetup for which ubi was already issued
		RewardsAlreadyIssued,
		/// Trying to claim UBI for a meetup where votes are not dependable
		VotesNotDependable,
		/// Overflow adding user to registry
		RegistryOverflow,
		/// CheckedMath operation error
		CheckedMath,
	}
}

impl<T: Config> Module<T> {
	fn register(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
		is_reputable: bool,
	) -> Result<(), Error<T>> {
		if <encointer_communities::Module<T>>::bootstrappers(cid).contains(&sender) {
			let participant_index = <BootstrapperCount>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<BootstrapperRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<BootstrapperIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<BootstrapperCount>::insert((cid, cindex), participant_index);
		} else if is_reputable {
			let participant_index = <ReputableCount>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<ReputableRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<ReputableIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<ReputableCount>::insert((cid, cindex), participant_index);
		} else if <Endorsees<T>>::contains_key((cid, cindex), &sender) {
			let participant_index = <EndorseeCount>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<EndorseeRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<EndorseeIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<EndorseeCount>::insert((cid, cindex), participant_index);
		} else {
			let participant_index = <NewbieCount>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<NewbieRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<NewbieIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<NewbieCount>::insert((cid, cindex), participant_index);
		}
		Ok(())
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

	fn purge_registry(cindex: CeremonyIndexType) {
		let cids = <encointer_communities::Module<T>>::community_identifiers();
		for cid in cids.iter() {
			<BootstrapperRegistry<T>>::remove_prefix((cid, cindex), None);
			<BootstrapperIndex<T>>::remove_prefix((cid, cindex), None);
			<BootstrapperCount>::insert((cid, cindex), 0);

			<ReputableRegistry<T>>::remove_prefix((cid, cindex), None);
			<ReputableIndex<T>>::remove_prefix((cid, cindex), None);
			<ReputableCount>::insert((cid, cindex), 0);

			<EndorseeRegistry<T>>::remove_prefix((cid, cindex), None);
			<EndorseeIndex<T>>::remove_prefix((cid, cindex), None);
			<EndorseeCount>::insert((cid, cindex), 0);

			<NewbieRegistry<T>>::remove_prefix((cid, cindex), None);
			<NewbieIndex<T>>::remove_prefix((cid, cindex), None);
			<NewbieCount>::insert((cid, cindex), 0);

			<AssignmentCounts>::insert((cid, cindex), AssignmentCount::default());

			Assignments::remove((cid, cindex));

			<Endorsees<T>>::remove_prefix((cid, cindex), None);
			<MeetupCount>::insert((cid, cindex), 0);
			<AttestationRegistry<T>>::remove_prefix((cid, cindex), None);
			<AttestationIndex<T>>::remove_prefix((cid, cindex), None);
			<AttestationCount>::insert((cid, cindex), 0);
			<MeetupParticipantCountVote<T>>::remove_prefix((cid, cindex), None);

			<IssuedRewards>::remove_prefix((cid, cindex), None);
		}
		debug!(target: LOG, "purged registry for ceremony {}", cindex);
	}

	fn generate_meetup_assignment_params(community_ceremony: CommunityCeremony) -> DispatchResult {
		let meetup_multiplier = 10u64;
		let assignment_count =
			Self::create_assignment_count(community_ceremony, meetup_multiplier)?;

		let num_meetups =
			checked_ceil_division(assignment_count.get_number_of_participants(), meetup_multiplier)
				.ok_or(Error::<T>::CheckedMath)?;

		Assignments::insert(
			community_ceremony,
			Assignment {
				bootstrappers_reputables: Self::generate_assignment_function_params(
					assignment_count.bootstrappers + assignment_count.reputables,
					num_meetups,
				),
				endorsees: Self::generate_assignment_function_params(
					assignment_count.endorsees,
					num_meetups,
				),
				newbies: Self::generate_assignment_function_params(
					assignment_count.newbies,
					num_meetups,
				),
				locations: Self::generate_location_assignment_params(community_ceremony),
			},
		);

		<AssignmentCounts>::insert(community_ceremony, assignment_count);
		<MeetupCount>::insert(community_ceremony, num_meetups);
		Ok(())
	}

	fn generate_location_assignment_params(
		community_ceremony: CommunityCeremony,
	) -> AssignmentParams {
		let num_locations =
			<encointer_communities::Module<T>>::get_locations(&community_ceremony.0).len() as u64;

		let mut random_source = RandomNumberGenerator::<T::Hashing>::new(
			// we don't need to pass a subject here, as this is only called once in a block.
			T::RandomnessSource::random_seed().0,
		);

		AssignmentParams {
			m: num_locations,
			s1: find_random_coprime_below(num_locations, &mut random_source),
			s2: find_prime_below(num_locations),
		}
	}

	fn create_assignment_count(
		community_ceremony: CommunityCeremony,
		meetup_multiplier: u64,
	) -> Result<AssignmentCount, Error<T>> {
		let num_locations =
			<encointer_communities::Module<T>>::get_locations(&community_ceremony.0).len() as u64;
		if num_locations == 0 {
			return Err(<Error<T>>::NoLocationsAvailable.into())
		}

		let num_assigned_bootstrappers = Self::bootstrapper_count(community_ceremony);
		let num_reputables = Self::reputable_count(community_ceremony);
		let max_num_meetups =
			min(num_locations, find_prime_below(num_assigned_bootstrappers + num_reputables));

		let mut available_slots = max_num_meetups * meetup_multiplier - num_assigned_bootstrappers;

		let num_assigned_reputables = min(num_reputables, available_slots);
		available_slots -= num_assigned_reputables;

		let num_assigned_endorsees = min(Self::endorsee_count(community_ceremony), available_slots);
		available_slots -= num_assigned_endorsees;

		let num_assigned_newbies = min(
			min(Self::newbie_count(community_ceremony), available_slots),
			(num_assigned_bootstrappers + num_assigned_reputables + num_assigned_endorsees) / 3,
		);

		Ok(AssignmentCount {
			bootstrappers: num_assigned_bootstrappers,
			reputables: num_assigned_reputables,
			endorsees: num_assigned_endorsees,
			newbies: num_assigned_newbies,
		})
	}

	fn generate_all_meetup_assignment_params() -> DispatchResult {
		let cids = <encointer_communities::Module<T>>::community_identifiers();
		let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
		for cid in cids.iter() {
			if let Err(e) = Self::generate_meetup_assignment_params((*cid, cindex)) {
				error!(
					target: LOG,
					"Could not generate meetup assignment params for cid: {:?}. {:?}", cid, e
				);
			}
		}
		Ok(())
	}

	fn mod_inv(a: i64, module: i64) -> i64 {
		let mut mn = (module, a);
		let mut xy = (0, 1);

		while mn.1 != 0 {
			xy = (xy.1, xy.0 - (mn.0 / mn.1) * xy.1);
			mn = (mn.1, mn.0 % mn.1);
		}

		while xy.0 < 0 {
			xy.0 += module;
		}
		xy.0
	}

	fn validate_equal_mapping(
		num_participants: u64,
		assignment_params: AssignmentParams,
		n: u64,
	) -> bool {
		if num_participants < 2 {
			return true
		}

		let mut meetup_index_count: Vec<u64> = vec![0; n as usize];
		let meetup_index_count_max =
			checked_ceil_division(num_participants - assignment_params.m, n).unwrap();
		for i in assignment_params.m..num_participants {
			let meetup_index = Self::assignment_fn(i, assignment_params, n).unwrap();
			meetup_index_count[meetup_index as usize] += 1;
			if meetup_index_count[meetup_index as usize] > meetup_index_count_max {
				return false
			}
		}
		true
	}

	fn get_random_nonzero_group_element(m: u64) -> u64 {
		let mut random_source =
			RandomNumberGenerator::<T::Hashing>::new(T::RandomnessSource::random_seed().0);

		// random number in [1, m-1]
		(random_source.pick_usize((m - 2) as usize) + 1) as u64
	}

	fn generate_assignment_function_params(
		num_participants: u64,
		num_meetups: u64,
	) -> AssignmentParams {
		let max_skips = 200;
		let m = find_prime_below(num_participants);
		let mut skip_count = 0;
		let mut s1 = Self::get_random_nonzero_group_element(m);
		let mut s2 = Self::get_random_nonzero_group_element(m);

		while skip_count <= max_skips {
			s1 = Self::get_random_nonzero_group_element(m);
			s2 = Self::get_random_nonzero_group_element(m);
			if Self::validate_equal_mapping(
				num_participants,
				AssignmentParams { m, s1, s2 },
				num_meetups,
			) {
				break
			} else {
				skip_count += 1;
			}
		}
		return AssignmentParams { m, s1, s2 }
	}

	fn assignment_fn_inverse(
		meetup_index: u64,
		assignment_params: AssignmentParams,
		n: u64,
		num_participants: u64,
	) -> Vec<ParticipantIndexType> {
		let mut result: Vec<ParticipantIndexType> = vec![];
		let mut max_index = (assignment_params.m as i64 - meetup_index as i64) / n as i64;
		// ceil
		if (assignment_params.m as i64 - meetup_index as i64).rem_euclid(n as i64) != 0 {
			max_index += 1;
		}

		for i in 0..max_index {
			let t1 = (n as i64 * i as i64 + meetup_index as i64 - assignment_params.s2 as i64)
				.rem_euclid(assignment_params.m as i64);
			let t2 = Self::mod_inv(assignment_params.s1 as i64, assignment_params.m as i64);
			let t3 = (t1 * t2).rem_euclid(assignment_params.m as i64);
			if t3 >= num_participants as i64 {
				continue
			}
			result.push(t3 as u64);
			if t3 < num_participants as i64 - assignment_params.m as i64 {
				result.push(t3 as u64 + assignment_params.m)
			}
		}
		result
	}

	/// Assigns a participant to a meetup.
	///
	/// Returns an error if the checked math operations fail.
	fn assignment_fn(
		participant_index: ParticipantIndexType,
		assignment_params: AssignmentParams,
		n: u64,
	) -> Result<MeetupIndexType, Error<T>> {
		let index = (participant_index
			.checked_mul(assignment_params.s1)
			.ok_or(Error::<T>::CheckedMath)?
			.checked_add(assignment_params.s2)
			.ok_or(Error::<T>::CheckedMath)? %
			assignment_params.m) %
			n;

		Ok(index)
	}

	fn get_meetup_index(
		community_ceremony: CommunityCeremony,
		participant: &T::AccountId,
	) -> Result<MeetupIndexType, Error<T>> {
		let meetup_count = Self::meetup_count(community_ceremony);

		let assignment = Self::assignments(community_ceremony);

		if <BootstrapperIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::bootstrapper_index(community_ceremony, &participant) - 1;
			return Ok(Self::assignment_fn(
				participant_index,
				assignment.bootstrappers_reputables,
				meetup_count,
			)? + 1)
		}
		if <ReputableIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::reputable_index(community_ceremony, &participant) - 1;
			return Ok(Self::assignment_fn(
				participant_index + Self::assignment_counts(community_ceremony).bootstrappers,
				assignment.bootstrappers_reputables,
				meetup_count,
			)? + 1)
		}

		if <EndorseeIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::endorsee_index(community_ceremony, &participant) - 1;
			return Ok(
				Self::assignment_fn(participant_index, assignment.endorsees, meetup_count)? + 1
			)
		}

		if <NewbieIndex<T>>::contains_key(community_ceremony, &participant) {
			let participant_index = Self::newbie_index(community_ceremony, &participant) - 1;
			return Ok(Self::assignment_fn(participant_index, assignment.newbies, meetup_count)? + 1)
		}
		Err(<Error<T>>::ParticipantIsNotRegistered.into())
	}

	fn get_meetup_participants(
		community_ceremony: CommunityCeremony,
		mut meetup_index: MeetupIndexType,
	) -> Vec<T::AccountId> {
		let mut result: Vec<T::AccountId> = vec![];
		let meetup_count = Self::meetup_count(community_ceremony);

		// meetup index conversion from 1 based to 0 based
		meetup_index -= 1;
		assert!(meetup_index < meetup_count);

		let params = Self::assignments(community_ceremony);

		let assigned = Self::assignment_counts(community_ceremony);

		let bootstrappers_reputables = Self::assignment_fn_inverse(
			meetup_index,
			params.bootstrappers_reputables,
			meetup_count,
			assigned.bootstrappers + assigned.reputables,
		);
		for p in bootstrappers_reputables {
			if p < assigned.bootstrappers {
				result.push(Self::bootstrapper_registry(community_ceremony, &(p + 1)));
			} else if p < assigned.bootstrappers + assigned.reputables {
				result.push(Self::reputable_registry(
					community_ceremony,
					&(p - assigned.bootstrappers + 1),
				));
			}
		}

		let endorsees = Self::assignment_fn_inverse(
			meetup_index,
			params.endorsees,
			meetup_count,
			assigned.endorsees,
		);
		for p in endorsees {
			if p < assigned.endorsees {
				result.push(Self::endorsee_registry(community_ceremony, &(p + 1)));
			}
		}

		let newbies = Self::assignment_fn_inverse(
			meetup_index,
			params.newbies,
			meetup_count,
			assigned.newbies,
		);
		for p in newbies {
			if p < assigned.newbies {
				result.push(Self::newbie_registry(community_ceremony, &(p + 1)));
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

	fn issue_rewards(participant: &T::AccountId, cid: &CommunityIdentifier) -> DispatchResult {
		if <encointer_scheduler::Module<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
			return Err(<Error<T>>::WrongPhaseForClaimingRewards.into())
		}

		let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index() - 1;
		let reward = Self::nominal_income(cid);
		let meetup_index = Self::get_meetup_index((*cid, cindex), participant)?;

		if <IssuedRewards>::contains_key((cid, cindex), meetup_index) {
			return Err(<Error<T>>::RewardsAlreadyIssued.into())
		}

		// first, evaluate votes on how many participants showed up
		let (n_confirmed, n_honest_participants) =
			match Self::ballot_meetup_n_votes(cid, cindex, meetup_index) {
				Some(nn) => nn,
				_ => return Err(<Error<T>>::VotesNotDependable.into()),
			};
		let meetup_participants = Self::get_meetup_participants((*cid, cindex), meetup_index);
		for participant in &meetup_participants {
			if Self::meetup_participant_count_vote((cid, cindex), &participant) != n_confirmed {
				debug!(
					target: LOG,
					"skipped participant because of wrong participant count vote: {:?}",
					participant
				);
				continue
			}
			let attestees = Self::attestation_registry(
				(cid, cindex),
				&Self::attestation_index((*cid, cindex), &participant),
			);
			if attestees.len() < (n_honest_participants - 1) as usize {
				debug!(
					target: LOG,
					"skipped participant because didn't testify for honest peers: {:?}",
					participant
				);
				continue
			}

			let mut was_attested_count = 0u32;
			for other_participant in &meetup_participants {
				if other_participant == participant {
					continue
				}
				let attestees_from_other = Self::attestation_registry(
					(cid, cindex),
					&Self::attestation_index((cid, cindex), &other_participant),
				);
				if attestees_from_other.contains(&participant) {
					was_attested_count += 1;
				}
			}

			if was_attested_count < (n_honest_participants - 1) {
				debug!(
					"skipped participant because of too few attestations ({}): {:?}",
					was_attested_count, participant
				);
				continue
			}

			trace!(target: LOG, "participant merits reward: {:?}", participant);
			if <encointer_balances::Module<T>>::issue(*cid, &participant, reward).is_ok() {
				<ParticipantReputation<T>>::insert(
					(cid, cindex),
					&participant,
					Reputation::VerifiedUnlinked,
				);
			}
		}
		<IssuedRewards>::insert((cid, cindex), meetup_index, ());
		info!(target: LOG, "issuing rewards completed");
		Ok(())
	}

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
				Some(idx) => n_vote_candidates[idx].1 += 1,
				_ => n_vote_candidates.insert(0, (this_vote, 1)),
			};
		}
		if n_vote_candidates.is_empty() {
			return None
		}
		// sort by descending vote count
		n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
		if n_vote_candidates[0].1 < 3 {
			return None
		}
		Some(n_vote_candidates[0])
	}

	pub fn get_meetup_location(
		cc: CommunityCeremony,
		meetup_idx: MeetupIndexType,
	) -> Option<Location> {
		let locations = <encointer_communities::Module<T>>::get_locations(&cc.0);
		let assignment_params = Self::assignments(cc).locations;
		let location_idx =
			Self::assignment_fn(meetup_idx, assignment_params, locations.len() as u64).ok()?;
		if (location_idx >= 0) && (location_idx < locations.len() as u64) {
			Some(locations[(location_idx) as usize])
		} else {
			None
		}
	}

	// this function only works during ATTESTING, so we're keeping it for private use
	fn get_meetup_time(cc: CommunityCeremony, meetup_idx: MeetupIndexType) -> Option<T::Moment> {
		if !(<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::ATTESTING) {
			return None
		}
		if meetup_idx == 0 {
			return None
		}
		let duration =
			<encointer_scheduler::Module<T>>::phase_durations(CeremonyPhaseType::ATTESTING);
		let next = <encointer_scheduler::Module<T>>::next_phase_timestamp();
		let mlocation = Self::get_meetup_location(cc, meetup_idx)?;
		let day = T::MomentsPerDay::get();
		let perdegree = day / T::Moment::from(360u32);
		let start = next - duration;
		// rounding to the lower integer degree. Max error: 240s = 4min
		let abs_lon: u32 = i64::lossy_from(mlocation.lon.abs()).saturated_into();
		let abs_lon_time = T::Moment::from(abs_lon) * perdegree;

		if mlocation.lon < Degree::from_num(0) {
			Some(start + day / T::Moment::from(2u32) + abs_lon_time)
		} else {
			Some(start + day / T::Moment::from(2u32) - abs_lon_time)
		}
	}

	/// Returns the community-specific nominal income if it is set. Otherwise returns the
	/// the ceremony reward defined in the genesis config.
	fn nominal_income(cid: &CommunityIdentifier) -> NominalIncome {
		encointer_communities::NominalIncome::try_get(cid)
			.unwrap_or_else(|_| Self::ceremony_reward())
	}

	#[cfg(test)]
	// only to be used by tests
	fn fake_reputation(cidcindex: CommunityCeremony, account: &T::AccountId, rep: Reputation) {
		<ParticipantReputation<T>>::insert(&cidcindex, account, rep);
	}
}

impl<T: Config> OnCeremonyPhaseChange for Module<T> {
	fn on_ceremony_phase_change(new_phase: CeremonyPhaseType) {
		match new_phase {
			CeremonyPhaseType::ASSIGNING => {
				Self::generate_all_meetup_assignment_params().ok();
			},
			CeremonyPhaseType::ATTESTING => {},
			CeremonyPhaseType::REGISTERING => {
				let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
				// Clean up with a time delay, such that participants can claim their UBI in the following cycle.
				if cindex > 2 {
					Self::purge_registry(cindex - 2);
				}
			},
		}
	}
}

mod math;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
