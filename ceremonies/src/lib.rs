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
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    storage::{StorageDoubleMap, StorageMap},
    traits::{Get, Randomness},
};
use frame_system::ensure_signed;
use log::{debug, info, trace, warn};
use rstd::{
    prelude::*,
    vec,
};
use sp_runtime::{
    traits::{CheckedSub, IdentifyAccount, Member, Verify},
    SaturatedConversion
};

use encointer_primitives::{
    balances::BalanceType,
    ceremonies::*,
    ceremonies::consts::{AMOUNT_NEWBIE_TICKETS, REPUTATION_LIFETIME},
    communities::{CommunityIdentifier, Degree, Location, LossyFrom, NominalIncome},
    scheduler::{CeremonyIndexType, CeremonyPhaseType},
    RandomNumberGenerator
};
use encointer_scheduler::OnCeremonyPhaseChange;
use frame_support::sp_std::cmp::min;

// Logger target
const LOG: &str = "encointer";

pub trait Config:
frame_system::Config
+ timestamp::Config
+ encointer_communities::Config
+ encointer_balances::Config
+ encointer_scheduler::Config
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    type Public: IdentifyAccount<AccountId=Self::AccountId>;
    type Signature: Verify<Signer=Self::Public> + Member + Decode + Encode;
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


        AssignedBootstrapperCount get(fn assigned_bootstrapper_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;
        AssignedReputableCount get(fn assigned_reputable_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;
        AssignedEndorseeCount get(fn assigned_endorsee_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;
        AssignedNewbieCount get(fn assigned_newbie_count): map hasher(blake2_128_concat) CommunityCeremony => ParticipantIndexType;


        AssignmentParamsBootstrappersReputables get(fn assignment_params_bootstrappers_reputables): map hasher(blake2_128_concat) CommunityCeremony => AssignmentParams;
        AssignmentParamsEndorsees get(fn assignment_params_endorsees): map hasher(blake2_128_concat) CommunityCeremony => AssignmentParams;
        AssignmentParamsNewbies get(fn assignment_params_newbies): map hasher(blake2_128_concat) CommunityCeremony => AssignmentParams;

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

            if <BootstrapperIndex<T>>::contains_key((cid, cindex), &sender) ||
                <ReputableIndex<T>>::contains_key((cid, cindex), &sender)||
                <EndorseeIndex<T>>::contains_key((cid, cindex), &sender)||
                <NewbieIndex<T>>::contains_key((cid, cindex), &sender)
            {
                return Err(<Error<T>>::ParticipantAlreadyRegistered.into());
            }

            let mut participant_has_reputation = false;

            if let Some(p) = proof {
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

                participant_has_reputation = true;
            };

            if <encointer_communities::Module<T>>::bootstrappers(cid)
                .contains(&sender) {
                let participant_index = <BootstrapperCount>::get((cid, cindex)).checked_add(1).
                    ok_or("[EncointerCeremonies]: Overflow adding reputable to registry")?;
                <BootstrapperRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
                <BootstrapperIndex<T>>::insert((cid, cindex), &sender, &participant_index);
                <BootstrapperCount>::insert((cid, cindex), participant_index);
            } else if participant_has_reputation {
                let participant_index = <ReputableCount>::get((cid, cindex)).checked_add(1).
                    ok_or("[EncointerCeremonies]: Overflow adding reputable to registry")?;
                <ReputableRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
                <ReputableIndex<T>>::insert((cid, cindex), &sender, &participant_index);
                <ReputableCount>::insert((cid, cindex), participant_index);

            } else if <Endorsees<T>>::contains_key((cid, cindex), &sender) {
                let participant_index = <EndorseeCount>::get((cid, cindex)).checked_add(1).
                    ok_or("[EncointerCeremonies]: Overflow adding endorsee to registry")?;
                <EndorseeRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
                <EndorseeIndex<T>>::insert((cid, cindex), &sender, &participant_index);
                <EndorseeCount>::insert((cid, cindex), participant_index);
            } else {
                let participant_index = <NewbieCount>::get((cid, cindex)).checked_add(1).
                    ok_or("[EncointerCeremonies]: Overflow adding newbie to registry")?;
                <NewbieRegistry<T>>::insert((cid, cindex), &participant_index, &sender);
                <NewbieIndex<T>>::insert((cid, cindex), &sender, &participant_index);
                <NewbieCount>::insert((cid, cindex), participant_index);
            }


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

            let mlocation = if let Some(l) = Self::get_meetup_location(&cid, meetup_index)
                { l } else { return Err(<Error<T>>::MeetupLocationNotFound.into()) };
            let mtime = if let Some(t) = Self::get_meetup_time(&cid, meetup_index)
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
    }
}

impl<T: Config> Module<T> {
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


            <AssignedBootstrapperCount>::insert((cid, cindex), 0);
            <AssignedReputableCount>::insert((cid, cindex), 0);
            <AssignedEndorseeCount>::insert((cid, cindex), 0);
            <AssignedNewbieCount>::insert((cid, cindex), 0);

            AssignmentParamsBootstrappersReputables::remove((cid, cindex));
            AssignmentParamsEndorsees::remove((cid, cindex));
            AssignmentParamsNewbies::remove((cid, cindex));


            <Endorsees<T>>::remove_prefix((cid, cindex), None);
            <MeetupCount>::insert((cid, cindex), 0);
            <AttestationRegistry<T>>::remove_prefix((cid, cindex), None);
            <AttestationIndex<T>>::remove_prefix((cid, cindex), None);
            <AttestationCount>::insert((cid, cindex), 0);
            <MeetupParticipantCountVote<T>>::remove_prefix((cid, cindex), None);
        }
        debug!(target: LOG, "purged registry for ceremony {}", cindex);
    }

    fn generate_meetup_assignment_params(community_ceremony: CommunityCeremony) -> DispatchResult {
        let meetup_multiplier = 10u64;
        let num_locations = <encointer_communities::Module<T>>::get_locations(&community_ceremony.0).len() as u64;
        let num_boostrappers = Self::bootstrapper_count(community_ceremony);
        let num_reputables = Self::reputable_count(community_ceremony);
        let num_endorsees = Self::endorsee_count(community_ceremony);
        let num_newbies = Self::newbie_count(community_ceremony);

        if num_locations == 0 {
            return Err(<Error<T>>::NoLocationsAvailable.into())
        }
        let mut num_meetups = min(num_locations, Self::find_prime_below(num_boostrappers + num_reputables));

        let num_assigned_bootstrappers = num_boostrappers;

        let mut available_slots = num_meetups * meetup_multiplier - num_assigned_bootstrappers;


        let num_assigned_reputables = min(num_reputables, available_slots);
        available_slots -= num_assigned_reputables;

        let num_assigned_endorsees = min(num_endorsees, available_slots);
        available_slots -= num_assigned_endorsees;

        let max_assigned_newbies = (num_assigned_bootstrappers + num_assigned_reputables + num_assigned_endorsees) / 3;
        let mut num_assigned_newbies = min(num_newbies, available_slots);
        num_assigned_newbies = min(num_assigned_newbies, max_assigned_newbies);

        let num_participants = num_assigned_bootstrappers + num_assigned_reputables + num_assigned_endorsees + num_assigned_newbies;

        num_meetups = num_participants / meetup_multiplier;

        // ceil
        if num_participants % meetup_multiplier != 0 {num_meetups += 1;}


        let assignment_params_bootstrappers_reputables = Self::generate_assignment_function_params(num_assigned_bootstrappers + num_assigned_reputables, num_meetups);
        let assignment_params_endorsees = Self::generate_assignment_function_params(num_assigned_endorsees, num_meetups);
        let assignment_params_newbies = Self::generate_assignment_function_params(num_assigned_newbies, num_meetups);

        AssignmentParamsBootstrappersReputables::insert(community_ceremony, assignment_params_bootstrappers_reputables);
        AssignmentParamsEndorsees::insert(community_ceremony, assignment_params_endorsees);
        AssignmentParamsNewbies::insert(community_ceremony, assignment_params_newbies);

        <AssignedBootstrapperCount>::insert(community_ceremony, num_assigned_bootstrappers);
        <AssignedReputableCount>::insert(community_ceremony, num_assigned_reputables);
        <AssignedEndorseeCount>::insert(community_ceremony, num_assigned_endorsees);
        <AssignedNewbieCount>::insert(community_ceremony, num_assigned_newbies);
        <MeetupCount>::insert(community_ceremony, num_meetups);
        Ok(())
    }

    fn generate_all_meetup_assignment_params() -> DispatchResult{
        let cids = <encointer_communities::Module<T>>::community_identifiers();
        let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
        for cid in cids.iter() {
            Self::generate_meetup_assignment_params((*cid, cindex)).ok();
        }
        Ok(())
    }

    fn is_prime(n: u64) -> bool {
        if n <= 3 {
            return n > 1;
        }
        if n % 2 == 0 || n % 3 == 0 {
            return false;
        }
        if n < 25 {
            return true;
        }
        let mut i: u64 = 5;
        while i.pow(2) <= n {
            if n % i == 0u64 || n % (i + 2u64) == 0u64 {
                return false;
            }
            i += 6u64;
        }
        return true;
    }


    fn find_prime_below(mut n: u64) -> u64 {
        if n <= 2 {
            return 2u64;
        }
        if n % 2 == 0 {
            n -= 1;
        }
        while n > 0 {
            if Self::is_prime(n) {
                return n;
            }
            n -= 2;
        }
        2u64
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

    fn validate_equal_mapping(num_participants: u64, m: u64, n: u64, s1: u64, s2:u64) -> bool{
        if num_participants < 2 {
            return true;
        }

        let mut meetup_index_count: Vec<u64> = vec![0; n as usize];
        let mut meetup_index_count_max = (num_participants - m) / n;
        if (num_participants - m) % n != 0 {meetup_index_count_max += 1;}
        for i in m..num_participants{
            let meetup_index = Self::assignment_fn(i, s1, s2, m, n);
            meetup_index_count[meetup_index as usize] += 1;
            if meetup_index_count[meetup_index as usize] > meetup_index_count_max{
                return false;
            }
        }
        true
    }


    fn get_random_nonzero_group_element(m: u64) -> u64{
        let mut random_source = RandomNumberGenerator::<T::Hashing>::new(
            T::RandomnessSource::random_seed().0
        );

        // random number in [1, m-1]
        (random_source.pick_usize((m - 2) as usize) + 1) as u64
    }

    fn generate_assignment_function_params(num_participants: u64, num_meetups: u64) -> AssignmentParams{
        let max_skips = 200;
        let m = Self::find_prime_below(num_participants);
        let mut skip_count = 0;
        let mut s1;
        let mut s2;
        loop {
            s1 = Self::get_random_nonzero_group_element(m);
            s2 = Self::get_random_nonzero_group_element(m);
            if Self::validate_equal_mapping(num_participants, m, num_meetups, s1, s2) {
                break;
            } else {
                skip_count += 1;
                if skip_count > max_skips {
                    break;
                }
            }
        }
        return AssignmentParams {
            m,
            s1,
            s2,
        };
    }

    fn assignment_fn_inverse(meetup_index: u64, s1: u64, s2: u64, m: u64, n: u64, num_participants: u64) -> Vec<ParticipantIndexType> {
        let mut result: Vec<ParticipantIndexType> = vec![];
        let mut max_index = (m as i64 - meetup_index as i64) / n as i64;
        // ceil
        if (m as i64 - meetup_index as i64).rem_euclid(n as i64) != 0 { max_index += 1; }


        for i in 0..max_index {
            let t1 = (n as i64 * i as i64 + meetup_index as i64  - s2 as i64).rem_euclid(m as i64);
            let t2 = Self::mod_inv(s1 as i64, m as i64);
            let t3 = (t1 * t2).rem_euclid(m as i64);
            if t3 >= num_participants as i64{
                continue
            }
            result.push(t3 as u64);
            if t3 < num_participants as i64 - m as i64 {
                result.push(t3 as u64 + m)
            }
        }
        result
    }

    fn assignment_fn(participant_index: ParticipantIndexType, s1: u64, s2: u64, m: u64, n: u64) -> MeetupIndexType {
        ((participant_index * s1 + s2) % m) % n
    }

    fn get_meetup_index(community_ceremony: CommunityCeremony, participant: &T::AccountId) -> Result<MeetupIndexType, Error<T>> {
        let meetup_count = Self::meetup_count(community_ceremony);

        if <BootstrapperIndex<T>>::contains_key(community_ceremony, &participant) {
            let AssignmentParams { m, s1, s2 } = Self::assignment_params_bootstrappers_reputables(community_ceremony);
            let participant_index = Self::bootstrapper_index(community_ceremony, &participant) - 1;
            return Ok(Self::assignment_fn(participant_index, s1, s2, m, meetup_count) + 1);
        }
        if <ReputableIndex<T>>::contains_key(community_ceremony, &participant) {
            let AssignmentParams { m, s1, s2 } = Self::assignment_params_bootstrappers_reputables(community_ceremony);
            let num_bootstrappers = Self::assigned_bootstrapper_count(community_ceremony);
            let participant_index = Self::reputable_index(community_ceremony, &participant) - 1;
            return Ok(Self::assignment_fn(participant_index + num_bootstrappers, s1, s2, m, meetup_count) + 1);
        }

        if <EndorseeIndex<T>>::contains_key(community_ceremony, &participant) {
            let AssignmentParams { m, s1, s2 } = Self::assignment_params_endorsees(community_ceremony);
            let participant_index = Self::endorsee_index(community_ceremony, &participant) - 1;
            return Ok(Self::assignment_fn(participant_index, s1, s2, m, meetup_count) + 1);
        }

        if <NewbieIndex<T>>::contains_key(community_ceremony, &participant) {
            let AssignmentParams { m, s1, s2 } = Self::assignment_params_newbies(community_ceremony);
            let participant_index = Self::newbie_index(community_ceremony, &participant) - 1;
            return Ok(Self::assignment_fn(participant_index, s1, s2, m, meetup_count) + 1);
        }
        Err(<Error<T>>::ParticipantIsNotRegistered.into())
    }

    fn get_meetup_participants(community_ceremony: CommunityCeremony, mut meetup_index: MeetupIndexType) -> Vec<T::AccountId> {
        let mut result: Vec<T::AccountId> = vec![];
        let meetup_count = Self::meetup_count(community_ceremony);

        // meetup index conversion from 1 based to 0 based
        meetup_index -=1;
        assert!(meetup_index < meetup_count);

        let params_br = Self::assignment_params_bootstrappers_reputables(community_ceremony);
        let params_e = Self::assignment_params_endorsees(community_ceremony);
        let params_n = Self::assignment_params_newbies(community_ceremony);

        let assigned_b = Self::assigned_bootstrapper_count(community_ceremony);
        let assigned_r = Self::assigned_reputable_count(community_ceremony);
        let assigned_e = Self::assigned_endorsee_count(community_ceremony);
        let assigned_n = Self::assigned_newbie_count(community_ceremony);

        let bootstrappers_reputables = Self::assignment_fn_inverse(meetup_index, params_br.s1, params_br.s2, params_br.m, meetup_count, assigned_b + assigned_r);
        for p in bootstrappers_reputables {
            if p < assigned_b {
                result.push(Self::bootstrapper_registry(community_ceremony, &(p + 1)));
            } else if p < assigned_b + assigned_r {
                result.push(Self::reputable_registry(community_ceremony, &(p - assigned_b + 1)));
            }
        }

        let endorsees = Self::assignment_fn_inverse(meetup_index, params_e.s1, params_e.s2, params_e.m, meetup_count, assigned_e);
        for p in endorsees {
            if p < assigned_e {
                result.push(Self::endorsee_registry(community_ceremony, &(p + 1)));
            }
        }

        let newbies = Self::assignment_fn_inverse(meetup_index, params_n.s1, params_n.s2, params_n.m, meetup_count, assigned_n);
        for p in newbies {
            if p < assigned_n {
                result.push(Self::newbie_registry(community_ceremony, &(p + 1)));
            }
        }

        return result;

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

    // this function takes O(n) for n meetups, so it should later be processed off-chain within
    // SubstraTEE-worker together with the entire registry
    // as this function can only be called by the ceremony state machine, it could actually work out fine
    // on-chain. It would just delay the next block once per ceremony cycle.
    fn issue_rewards() {
        if <encointer_scheduler::Module<T>>::current_phase() != CeremonyPhaseType::REGISTERING {
            return;
        }
        let cids = <encointer_communities::Module<T>>::community_identifiers();
        for cid in cids.iter() {
            let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index() - 1;
            let meetup_count = Self::meetup_count((cid, cindex));
            let reward = Self::nominal_income(cid);

            for m in 1..=meetup_count {
                // first, evaluate votes on how many participants showed up
                let (n_confirmed, n_honest_participants) =
                    match Self::ballot_meetup_n_votes(cid, cindex, m) {
                        Some(nn) => nn,
                        _ => {
                            warn!(
                                target: LOG,
                                "ignoring meetup {} because votes are not dependable",
                                m
                            );
                            continue;
                        }
                    };
                let meetup_participants = Self::get_meetup_participants((*cid, cindex), m);
                for p in &meetup_participants {
                    if Self::meetup_participant_count_vote((cid, cindex), &p) != n_confirmed {
                        debug!(
                            target: LOG,
                            "skipped participant because of wrong participant count vote: {:?}",
                            p
                        );
                        continue;
                    }
                    let attestees = Self::attestation_registry(
                        (cid, cindex),
                        &Self::attestation_index((*cid, cindex), &p),
                    );
                    if attestees.len() < (n_honest_participants - 1) as usize {
                        debug!(
                            target: LOG,
                            "skipped participant because didn't testify for honest peers: {:?}",
                            p
                        );
                        continue;
                    }

                    let mut was_attested_count = 0u32;
                    for other_participant in &meetup_participants {
                        if other_participant == p {
                            continue;
                        }
                        let attestees_from_other = Self::attestation_registry(
                            (cid, cindex),
                            &Self::attestation_index((cid, cindex), &other_participant),
                        );
                        if attestees_from_other.contains(&p) {
                            was_attested_count += 1;
                        }
                    }

                    if was_attested_count < (n_honest_participants - 1)
                    {
                        debug!(
                            "skipped participant because of too few attestations ({}): {:?}",
                            was_attested_count,
                            p
                        );
                        continue;
                    }

                    trace!(target: LOG, "participant merits reward: {:?}", p);
                    if <encointer_balances::Module<T>>::issue(*cid, &p, reward).is_ok() {
                        <ParticipantReputation<T>>::insert(
                            (cid, cindex),
                            &p,
                            Reputation::VerifiedUnlinked,
                        );
                    }
                }
            }
        }
        info!(target: LOG, "issuing rewards completed");
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
            return None;
        }
        // sort by descending vote count
        n_vote_candidates.sort_by(|a, b| b.1.cmp(&a.1));
        if n_vote_candidates[0].1 < 3 {
            return None;
        }
        Some(n_vote_candidates[0])
    }

    pub fn get_meetup_location(
        cid: &CommunityIdentifier,
        meetup_idx: MeetupIndexType,
    ) -> Option<Location> {
        let locations = <encointer_communities::Module<T>>::get_locations(cid);
        if (meetup_idx > 0) && (meetup_idx <= locations.len() as MeetupIndexType) {
            Some(locations[(meetup_idx - 1) as usize])
        } else {
            None
        }
    }

    // this function only works during ATTESTING, so we're keeping it for private use
    fn get_meetup_time(
        cid: &CommunityIdentifier,
        meetup_idx: MeetupIndexType,
    ) -> Option<T::Moment> {
        if !(<encointer_scheduler::Module<T>>::current_phase() == CeremonyPhaseType::ATTESTING) {
            return None;
        }
        if meetup_idx == 0 {
            return None;
        }
        let duration =
            <encointer_scheduler::Module<T>>::phase_durations(CeremonyPhaseType::ATTESTING);
        let next = <encointer_scheduler::Module<T>>::next_phase_timestamp();
        let mlocation = Self::get_meetup_location(&cid, meetup_idx)?;
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
            }
            CeremonyPhaseType::ATTESTING => {}
            CeremonyPhaseType::REGISTERING => {
                Self::issue_rewards();
                let cindex = <encointer_scheduler::Module<T>>::current_ceremony_index();
                Self::purge_registry(cindex - 1);
            }
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod mock;
#[cfg(test)]
#[macro_use]
extern crate approx;
