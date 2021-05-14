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

use codec::{Decode, Encode};
use sp_core::{Pair, RuntimeDebug, H256};
use sp_runtime::traits::{BlakeTwo256, Hash, IdentifyAccount, Verify};

use crate::communities::{CommunityIdentifier, Location};
use crate::scheduler::CeremonyIndexType;

pub type ParticipantIndexType = u64;
pub type MeetupIndexType = u64;
pub type AttestationIndexType = u64;
pub type CommunityCeremony = (CommunityIdentifier, CeremonyIndexType);

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum Reputation {
    // no attestations for attendance claim
    Unverified,
    // no attestation yet but linked to reputation
    UnverifiedReputable,
    // verified former attendance that has not yet been linked to a new registration
    VerifiedUnlinked,
    // verified former attendance that has already been linked to a new registration
    VerifiedLinked,
}

impl Default for Reputation {
    fn default() -> Self {
        Reputation::Unverified
    }
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ClaimOfAttendance<Signature, AccountId, Moment> {
    pub claimant_public: AccountId,
    pub ceremony_index: CeremonyIndexType,
    pub community_identifier: CommunityIdentifier,
    pub meetup_index: MeetupIndexType,
    pub location: Location,
    pub timestamp: Moment,
    pub number_of_participants_confirmed: u32,
    pub claimant_signature: Option<Signature>,
}

impl<Signature, AccountId, Moment> ClaimOfAttendance<Signature, AccountId, Moment> {
    pub fn new_signed(
        claimant_public: AccountId,
        ceremony_index: CeremonyIndexType,
        community_identifier: CommunityIdentifier,
        meetup_index: MeetupIndexType,
        location: Location,
        timestamp: Moment,
        number_of_participants_confirmed: u32,
        claimant_signature: Signature,
    ) -> Self {
        Self {
            claimant_public,
            ceremony_index,
            community_identifier,
            meetup_index,
            location,
            timestamp,
            number_of_participants_confirmed,
            claimant_signature: Some(claimant_signature),
        }
    }

    pub fn new_unsigned(
        claimant_public: AccountId,
        ceremony_index: CeremonyIndexType,
        community_identifier: CommunityIdentifier,
        meetup_index: MeetupIndexType,
        location: Location,
        timestamp: Moment,
        number_of_participants_confirmed: u32,
    ) -> Self {
        Self {
            claimant_public,
            ceremony_index,
            community_identifier,
            meetup_index,
            location,
            timestamp,
            number_of_participants_confirmed,
            claimant_signature: None,
        }
    }

    pub fn set_claimant(self, claimant: AccountId) -> Self {
        let mut claim_mut = self;
        claim_mut.claimant_public = claimant;
        claim_mut
    }

    pub fn set_participant_count(self, count: u32) -> Self {
        let mut claim_mut = self;
        claim_mut.number_of_participants_confirmed = count;
        claim_mut
    }
}

impl<Signature, AccountId: Encode, Moment: Encode + Copy>
    ClaimOfAttendance<Signature, AccountId, Moment>
{
    pub fn payload_encoded(&self) -> Vec<u8> {
        (
            self.claimant_public.encode(),
            self.ceremony_index,
            self.community_identifier,
            self.meetup_index,
            self.location,
            self.timestamp,
            self.number_of_participants_confirmed,
        )
            .encode()
    }

    pub fn sign<P>(self, pair: &P) -> Self
    where
    P: Pair,
    Signature: From<P::Signature>
    {
        let mut claim_mut = self;
        claim_mut.claimant_signature = Some(Signature::from(pair.sign(&claim_mut.payload_encoded()[..])));
        claim_mut
    }
}

impl<Signature, AccountId, Moment> ClaimOfAttendance<Signature, AccountId, Moment> {
    pub fn verify(&self) -> bool
    where
        Signature: Verify,
        <Signature as Verify>::Signer: IdentifyAccount<AccountId =AccountId>,
        AccountId: Clone + Encode,
        Moment: Copy + Encode,
    {
        self.claimant_signature
            .as_ref()
            .map(|sig| sig.verify(&self.payload_encoded()[..], &self.claimant_public))
            .unwrap_or(false)
    }
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfAttendance<Signature, AccountId> {
    pub prover_public: AccountId,
    pub ceremony_index: CeremonyIndexType,
    pub community_identifier: CommunityIdentifier,
    pub attendee_public: AccountId,
    pub attendee_signature: Signature,
}

impl<Signature, AccountId: Clone + Encode> ProofOfAttendance<Signature, AccountId> {
    /// get the hash of the proof without the attendee signature,
    /// as the signature is non-deterministic.
    pub fn hash(&self) -> H256 {
        (
            self.prover_public.clone(),
            self.ceremony_index,
            self.community_identifier,
            self.attendee_public.clone(),
        )
            .using_encoded(BlakeTwo256::hash)
    }
}

pub mod consts {
    pub const REPUTATION_LIFETIME: u32 = 1;
    pub const AMOUNT_NEWBIE_TICKETS: u8 = 50;
}
