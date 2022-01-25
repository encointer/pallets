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
use sp_std::{prelude::*, vec};

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
		type Public: IdentifyAccount<AccountId = Self::AccountId>;
		type Signature: Verify<Signer = Self::Public> + Member + Decode + Encode + TypeInfo;
		type RandomnessSource: Randomness<Self::Hash, Self::BlockNumber>;
		#[pallet::constant]
		type ReputationLifetime: Get<u32>;
		#[pallet::constant]
		type AmountNewbieTickets: Get<u8>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn grant_reputation(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			reputable: T::AccountId,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				sender == <encointer_scheduler::Module<T>>::ceremony_master(),
				Error::<T>::AuthorizationRequired
			);
			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
			<ParticipantReputation<T>>::insert(
				&(cid, cindex - 1),
				reputable,
				Reputation::VerifiedUnlinked,
			); //safe; cindex comes from within, will not overflow at +1/d
			info!(target: LOG, "granting reputation to {:?}", sender);
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn register_participant(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			proof: Option<ProofOfAttendance<T::Signature, T::AccountId>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::REGISTERING,
				Error::<T>::RegisteringPhaseRequired
			);

			ensure!(
				<encointer_communities::Pallet<T>>::community_identifiers().contains(&cid),
				Error::<T>::InexistentCommunity
			);

			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();

			if Self::is_registered(cid, cindex, &sender) {
				return Err(<Error<T>>::ParticipantAlreadyRegistered.into())
			}

			if let Some(p) = &proof {
				// we accept proofs from other communities as well. no need to ensure cid
				ensure!(sender == p.prover_public, Error::<T>::WrongProofSubject);
				ensure!(p.ceremony_index < cindex, Error::<T>::ProofAcausal);
				ensure!(
					p.ceremony_index >=
						cindex.checked_sub(T::ReputationLifetime::get()).unwrap_or(0),
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

			Self::register(cid, cindex, &sender, proof.is_some())?;

			debug!(target: LOG, "registered participant: {:?}", sender);
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn attest_claims(
			origin: OriginFor<T>,
			claims: Vec<ClaimOfAttendance<T::Signature, T::AccountId, T::Moment>>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(
				<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::ATTESTING,
				Error::<T>::AttestationPhaseRequired
			);
			let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
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
				"meetup {} at location {:?} should happen at {:?} for cid {:?}",
				meetup_index,
				mlocation,
				mtime,
				cid
			);

			for claim in claims.iter() {
				let claimant = &claim.claimant_public;
				if claimant == &sender {
					warn!(target: LOG, "ignoring claim that is from sender: {:?}", claimant);
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
			debug!(target: LOG, "successfully registered {} claims", verified_attestees.len());
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
					T::AmountNewbieTickets::get(),
				Error::<T>::NoMoreNewbieTickets
			);

			let mut cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
			if <encointer_scheduler::Module<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
				cindex += 1; //safe; cindex comes from within, will not overflow at +1/d
			}
			ensure!(
				!<Endorsees<T>>::contains_key((cid, cindex), &newbie),
				Error::<T>::AlreadyEndorsed
			);

			<BurnedBootstrapperNewbieTickets<T>>::mutate(&cid, sender, |b| *b += 1); // safe; limited by AMOUNT_NEWBIE_TICKETS
			debug!(target: LOG, "endorsed newbie: {:?}", newbie);
			<Endorsees<T>>::insert((cid, cindex), newbie, ());
			<EndorseesCount<T>>::mutate((cid, cindex), |c| *c += 1); // safe; limited by AMOUNT_NEWBIE_TICKETS
			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			Self::issue_rewards(&sender, &cid)
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Participant registered for next ceremony [who]
		ParticipantRegistered(T::AccountId),
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
		ValueQuery,
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
		ValueQuery,
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
		ValueQuery,
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
		ValueQuery,
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
		ValueQuery,
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

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config>
// TODO_MAYBE_WHERE_CLAUSE
	{
		#[doc = " the default UBI for a ceremony attendee if no community specific value is set."]
		pub ceremony_reward: BalanceType,
		pub location_tolerance: u32,
		pub time_tolerance: T::Moment,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T>
	// TODO_MAYBE_WHERE_CLAUSE
	{
		fn default() -> Self {
			Self {
				ceremony_reward: Default::default(),
				location_tolerance: Default::default(),
				time_tolerance: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			{
				let data = &self.ceremony_reward;
				let v: &BalanceType = data;
				<CeremonyReward<T> as frame_support::storage::StorageValue<BalanceType>>::put::<
					&BalanceType,
				>(v);
			}
			{
				let data = &self.location_tolerance;
				let v: &u32 = data;
				<LocationTolerance<T> as frame_support::storage::StorageValue<u32>>::put::<&u32>(v);
			}
			{
				let data = &self.time_tolerance;
				let v: &T::Moment = data;
				<TimeTolerance<T> as frame_support::storage::StorageValue<T::Moment>>::put::<
					&T::Moment,
				>(v);
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_reputations() -> Vec<(CommunityCeremony, T::AccountId, Reputation)> {
		return ParticipantReputation::<T>::iter().collect()
	}

	fn register(
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		sender: &T::AccountId,
		is_reputable: bool,
	) -> Result<(), Error<T>> {
		if <encointer_communities::Pallet<T>>::bootstrappers(cid).contains(&sender) {
			let participant_index = <BootstrapperCount<T>>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<BootstrapperRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<BootstrapperIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<BootstrapperCount<T>>::insert((cid, cindex), participant_index);
		} else if is_reputable {
			let participant_index = <ReputableCount<T>>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<ReputableRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<ReputableIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<ReputableCount<T>>::insert((cid, cindex), participant_index);
		} else if <Endorsees<T>>::contains_key((cid, cindex), &sender) {
			let participant_index = <EndorseeCount<T>>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<EndorseeRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<EndorseeIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<EndorseeCount<T>>::insert((cid, cindex), participant_index);
		} else {
			let participant_index = <NewbieCount<T>>::get((cid, cindex))
				.checked_add(1)
				.ok_or(Error::<T>::RegistryOverflow)?;
			<NewbieRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
			<NewbieIndex<T>>::insert((cid, cindex), &sender, &participant_index);
			<NewbieCount<T>>::insert((cid, cindex), participant_index);
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
		let cids = <encointer_communities::Pallet<T>>::community_identifiers();
		for cid in cids.iter() {
			<BootstrapperRegistry<T>>::remove_prefix((cid, cindex), None);
			<BootstrapperIndex<T>>::remove_prefix((cid, cindex), None);
			<BootstrapperCount<T>>::insert((cid, cindex), 0);

			<ReputableRegistry<T>>::remove_prefix((cid, cindex), None);
			<ReputableIndex<T>>::remove_prefix((cid, cindex), None);
			<ReputableCount<T>>::insert((cid, cindex), 0);

			<EndorseeRegistry<T>>::remove_prefix((cid, cindex), None);
			<EndorseeIndex<T>>::remove_prefix((cid, cindex), None);
			<EndorseeCount<T>>::insert((cid, cindex), 0);

			<NewbieRegistry<T>>::remove_prefix((cid, cindex), None);
			<NewbieIndex<T>>::remove_prefix((cid, cindex), None);
			<NewbieCount<T>>::insert((cid, cindex), 0);

			<AssignmentCounts<T>>::insert((cid, cindex), AssignmentCount::default());

			Assignments::<T>::remove((cid, cindex));

			<Endorsees<T>>::remove_prefix((cid, cindex), None);
			<MeetupCount<T>>::insert((cid, cindex), 0);
			<AttestationRegistry<T>>::remove_prefix((cid, cindex), None);
			<AttestationIndex<T>>::remove_prefix((cid, cindex), None);
			<AttestationCount<T>>::insert((cid, cindex), 0);
			<MeetupParticipantCountVote<T>>::remove_prefix((cid, cindex), None);

			<IssuedRewards<T>>::remove_prefix((cid, cindex), None);
		}
		debug!(target: LOG, "purged registry for ceremony {}", cindex);
	}

	fn generate_meetup_assignment_params(
		community_ceremony: CommunityCeremony,
		random_source: &mut RandomNumberGenerator<T::Hashing>,
	) -> DispatchResult {
		let meetup_multiplier = 10u64;
		let assignment_count =
			Self::create_assignment_count(community_ceremony, meetup_multiplier)?;

		let num_meetups =
			checked_ceil_division(assignment_count.get_number_of_participants(), meetup_multiplier)
				.ok_or(Error::<T>::CheckedMath)?;

		<Assignments<T>>::insert(
			community_ceremony,
			Assignment {
				bootstrappers_reputables: generate_assignment_function_params(
					assignment_count.bootstrappers + assignment_count.reputables,
					num_meetups,
					random_source,
				),
				endorsees: generate_assignment_function_params(
					assignment_count.endorsees,
					num_meetups,
					random_source,
				),
				newbies: generate_assignment_function_params(
					assignment_count.newbies,
					num_meetups,
					random_source,
				),
				locations: Self::generate_location_assignment_params(
					community_ceremony,
					random_source,
				),
			},
		);

		<AssignmentCounts<T>>::insert(community_ceremony, assignment_count);
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

	fn create_assignment_count(
		community_ceremony: CommunityCeremony,
		meetup_multiplier: u64,
	) -> Result<AssignmentCount, Error<T>> {
		let num_locations =
			<encointer_communities::Pallet<T>>::get_locations(&community_ceremony.0).len() as u64;
		if num_locations == 0 {
			return Err(<Error<T>>::NoLocationsAvailable.into())
		}

		let num_assigned_bootstrappers = Self::bootstrapper_count(community_ceremony);
		let num_reputables = Self::reputable_count(community_ceremony);
		let max_num_meetups =
			min(num_locations, find_prime_below(num_assigned_bootstrappers + num_reputables));

		let mut available_slots =
			max_num_meetups.checked_mul(meetup_multiplier).ok_or(Error::<T>::CheckedMath)? -
				num_assigned_bootstrappers; //safe; number of assigned bootstrappers <= max_num_meetups <=num_assigned_bootstrappers + num_reputables

		let num_assigned_reputables = min(num_reputables, available_slots);
		available_slots -= num_assigned_reputables; //safe; given by minimum above

		let num_assigned_endorsees = min(Self::endorsee_count(community_ceremony), available_slots);
		available_slots -= num_assigned_endorsees; //safe; given by minimum above

		let num_assigned_newbies = min(
			min(Self::newbie_count(community_ceremony), available_slots),
			(num_assigned_bootstrappers + num_assigned_reputables + num_assigned_endorsees) / 3, //safe; sum equals total
		);

		Ok(AssignmentCount {
			bootstrappers: num_assigned_bootstrappers,
			reputables: num_assigned_reputables,
			endorsees: num_assigned_endorsees,
			newbies: num_assigned_newbies,
		})
	}

	fn generate_all_meetup_assignment_params() {
		let cids = <encointer_communities::Pallet<T>>::community_identifiers();
		let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();

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
				result.push(Self::bootstrapper_registry(community_ceremony, &(p + 1))); //safe; small number per meetup
			} else if p < assigned.bootstrappers + assigned.reputables {
				result.push(Self::reputable_registry(
					community_ceremony,
					&(p - assigned.bootstrappers + 1), //safe; small number per meetup
				));
			}
		}

		let endorsees =
			assignment_fn_inverse(meetup_index, params.endorsees, meetup_count, assigned.endorsees);
		for p in endorsees {
			if p < assigned.endorsees {
				result.push(Self::endorsee_registry(community_ceremony, &(p + 1))); //safe; <= number of meetup participants
			}
		}

		let newbies =
			assignment_fn_inverse(meetup_index, params.newbies, meetup_count, assigned.newbies);
		for p in newbies {
			if p < assigned.newbies {
				result.push(Self::newbie_registry(community_ceremony, &(p + 1))); //safe; <= number of meetup participants
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

	fn issue_rewards(
		participant: &T::AccountId,
		cid: &CommunityIdentifier,
	) -> DispatchResultWithPostInfo {
		if <encointer_scheduler::Module<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
			return Err(<Error<T>>::WrongPhaseForClaimingRewards.into())
		}

		let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index() - 1; //safe; cindex comes from within
		let reward = Self::nominal_income(cid);
		let meetup_index = Self::get_meetup_index((*cid, cindex), participant)
			.ok_or(<Error<T>>::ParticipantIsNotRegistered)?;

		if <IssuedRewards<T>>::contains_key((cid, cindex), meetup_index) {
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
					was_attested_count += 1; // <= number of meetup participants
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
			if <encointer_balances::Pallet<T>>::issue(*cid, &participant, reward).is_ok() {
				<ParticipantReputation<T>>::insert(
					(cid, cindex),
					&participant,
					Reputation::VerifiedUnlinked,
				);
			}
		}
		<IssuedRewards<T>>::insert((cid, cindex), meetup_index, ());
		info!(target: LOG, "issuing rewards completed");
		Ok(().into())
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
				Some(idx) => n_vote_candidates[idx].1 += 1, //safe; <= number of candidates
				_ => n_vote_candidates.insert(0, (this_vote, 1)),
			};
		}
		if n_vote_candidates.is_empty() {
			return None
		}
		// sort by descending vote count
		n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
		if n_vote_candidates[0].1 < 3 {
			//safe; n_vote_candidate not empty checked above
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
		if !(<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::ATTESTING) {
			return None
		}

		let duration =
			<encointer_scheduler::Module<T>>::phase_durations(CeremonyPhaseType::ATTESTING);
		let next = <encointer_scheduler::Module<T>>::next_phase_timestamp();
		let start = next - duration;

		Some(meetup_time(location, start, T::MomentsPerDay::get()))
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
				let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
				// Clean up with a time delay, such that participants can claim their UBI in the following cycle.
				if cindex > T::ReputationLifetime::get() {
					Self::purge_registry(cindex - T::ReputationLifetime::get() - 1);
				}
			},
		}
	}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
