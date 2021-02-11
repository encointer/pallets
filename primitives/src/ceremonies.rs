use codec::{Decode, Encode};
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::{BlakeTwo256, Hash};

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
pub struct Attestation<Signature, AccountId, Moment> {
    pub claim: ClaimOfAttendance<AccountId, Moment>,
    pub signature: Signature,
    pub public: AccountId,
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ClaimOfAttendance<AccountId, Moment> {
    pub claimant_public: AccountId,
    pub ceremony_index: CeremonyIndexType,
    pub community_identifier: CommunityIdentifier,
    pub meetup_index: MeetupIndexType,
    pub location: Location,
    pub timestamp: Moment,
    pub number_of_participants_confirmed: u32,
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
            .using_encoded(|d| BlakeTwo256::hash(&d).into())
    }
}

pub mod consts {
    pub const REPUTATION_LIFETIME: u32 = 1;
    pub const AMOUNT_NEWBIE_TICKETS: u8 = 50;
}
