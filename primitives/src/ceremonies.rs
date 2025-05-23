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

pub use crate::scheduler::CeremonyIndexType;
use crate::{
	communities::{CommunityIdentifier, Location},
	scheduler::CeremonyPhaseType,
};
use parity_scale_codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "std", feature = "full_crypto"))]
use sp_core::Pair;
use sp_core::RuntimeDebug;
use sp_runtime::traits::{IdentifyAccount, Verify};
#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

pub type ParticipantIndexType = u64;
pub type MeetupIndexType = u64;
pub type MeetupParticipantIndexType = u8;
pub type AttestationIndexType = u64;
pub type CommunityCeremony = (CommunityIdentifier, CeremonyIndexType);
pub type InactivityTimeoutType = u32;
pub type EndorsementTicketsType = u8;

/// reputation lifetime may not be longer than CeremonyIndexShort::MAX, otherwise double-using
/// reputation is possible. therefore, we restrict the type to u8
pub type ReputationLifetimeType = u32;
pub type MeetupTimeOffsetType = i32;
pub type MeetupData<AccountId, Moment> =
	(CeremonyIndexType, MeetupIndexType, Vec<AccountId>, Location, Moment);
pub type ReputationCountType = u128;

#[derive(
	Default,
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum Reputation {
	// no attestations for attendance claim
	#[default]
	Unverified,
	// no attestation yet but linked to reputation
	UnverifiedReputable,
	// verified former attendance that has not yet been linked to a new registration
	VerifiedUnlinked,
	// verified former attendance that has already been linked to a new registration
	VerifiedLinked(CeremonyIndexType),
}

impl Reputation {
	pub fn is_verified(&self) -> bool {
		matches!(self, Self::VerifiedLinked(_) | Self::VerifiedUnlinked)
	}

	pub fn is_verified_and_unlinked_for_cindex(&self, cindex: CeremonyIndexType) -> bool {
		match self {
			Self::VerifiedUnlinked => true,
			Self::VerifiedLinked(c) => *c != cindex,
			_ => false,
		}
	}
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum ParticipantType {
	// participant is a bootstrapper for this community
	Bootstrapper,
	// participant has recently been attested as a person
	Reputable,
	// participant has been endorsed by a bootstrapper
	Endorsee,
	// participant has no reputation yet
	Newbie,
}

#[derive(
	Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
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
	#[allow(clippy::too_many_arguments)]
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

impl<Signature, AccountId: Clone + Encode, Moment: Encode + Copy>
	ClaimOfAttendance<Signature, AccountId, Moment>
{
	pub fn payload_encoded(&self) -> Vec<u8> {
		(
			self.claimant_public.clone(),
			self.ceremony_index,
			self.community_identifier,
			self.meetup_index,
			self.location,
			self.timestamp,
			self.number_of_participants_confirmed,
		)
			.encode()
	}

	#[cfg(any(feature = "std", feature = "full_crypto"))]
	pub fn sign<P>(self, pair: &P) -> Self
	where
		P: sp_core::Pair,
		Signature: From<P::Signature>,
	{
		let mut claim_mut = self;
		claim_mut.claimant_signature =
			Some(Signature::from(pair.sign(&claim_mut.payload_encoded()[..])));
		claim_mut
	}
}

impl<Signature, AccountId, Moment> ClaimOfAttendance<Signature, AccountId, Moment> {
	pub fn verify_signature(&self) -> bool
	where
		Signature: Verify,
		<Signature as Verify>::Signer: IdentifyAccount<AccountId = AccountId>,
		AccountId: Clone + Encode,
		Moment: Copy + Encode,
	{
		self.claimant_signature
			.as_ref()
			.map(|sig| sig.verify(&self.payload_encoded()[..], &self.claimant_public))
			.unwrap_or(false)
	}
}

/// Reputation that is linked to a specific community
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct CommunityReputation {
	pub community_identifier: CommunityIdentifier,
	pub reputation: Reputation,
}

impl CommunityReputation {
	pub fn new(community_identifier: CommunityIdentifier, reputation: Reputation) -> Self {
		Self { community_identifier, reputation }
	}
}

pub type AccountIdFor<Signature> = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct ProofOfAttendance<Signature, AccountId> {
	pub prover_public: AccountId,
	pub ceremony_index: CeremonyIndexType,
	pub community_identifier: CommunityIdentifier,
	pub attendee_public: AccountId,
	pub attendee_signature: Signature,
}

impl<Signature: Verify> ProofOfAttendance<Signature, AccountIdFor<Signature>>
where
	AccountIdFor<Signature>: Clone + Encode,
{
	pub fn verify_signature(&self) -> bool {
		self.attendee_signature.verify(
			&(self.prover_public.clone(), self.ceremony_index).encode()[..],
			&self.attendee_public,
		)
	}

	#[cfg(any(feature = "std", feature = "full_crypto"))]
	pub fn signed<Signer>(
		prover_public: AccountIdFor<Signature>,
		cid: CommunityIdentifier,
		cindex: CeremonyIndexType,
		attendee: &Signer,
	) -> Self
	where
		Signer: Pair,
		Signature: From<Signer::Signature>,
		AccountIdFor<Signature>: From<Signer::Public>,
	{
		let msg = (prover_public.clone(), cindex);
		ProofOfAttendance {
			prover_public,
			community_identifier: cid,
			ceremony_index: cindex,
			attendee_public: attendee.public().into(),
			attendee_signature: attendee.sign(&msg.encode()).into(),
		}
	}
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Default,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AssignmentCount {
	pub bootstrappers: ParticipantIndexType,
	pub reputables: ParticipantIndexType,
	pub endorsees: ParticipantIndexType,
	pub newbies: ParticipantIndexType,
}

impl AssignmentCount {
	pub fn get_number_of_participants(&self) -> u64 {
		self.bootstrappers + self.reputables + self.endorsees + self.newbies
	}
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Default,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Assignment {
	pub bootstrappers_reputables: AssignmentParams,
	pub endorsees: AssignmentParams,
	pub newbies: AssignmentParams,
	pub locations: AssignmentParams,
}

// Todo: abstract AssignmentParams trait and use two different structs: AssignmentParams,
// LocationAssignmentParams
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Default,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AssignmentParams {
	/// Random prime below number of meetup participants. For locations this is the amount of
	/// locations.
	pub m: u64,
	/// First random group element in the interval (0, m). For locations this is a random coprime <
	/// m.
	pub s1: u64,
	/// Second random group element in the interval (0, m). For locations the closest prime to m,
	/// with s2 < m.
	pub s2: u64,
}

pub mod consts {
	/// Dirty bit key for reputation offchain storage
	pub const REPUTATION_CACHE_DIRTY_KEY: &[u8] = b"reputation_cache_dirty";
	pub const STORAGE_REPUTATION_KEY: &[u8; 10] = b"reputation";
}

pub fn reputation_cache_key<Account: Encode>(account: &Account) -> Vec<u8> {
	(consts::STORAGE_REPUTATION_KEY, account).encode()
}

pub fn reputation_cache_dirty_key<Account: Encode>(account: &Account) -> Vec<u8> {
	(consts::REPUTATION_CACHE_DIRTY_KEY, account).encode()
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AggregatedAccountDataPersonal<AccountId, Moment> {
	pub participant_type: ParticipantType,
	pub meetup_index: Option<MeetupIndexType>,
	pub meetup_location_index: Option<MeetupIndexType>,
	pub meetup_time: Option<Moment>,
	pub meetup_registry: Option<Vec<AccountId>>,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AggregatedAccountDataGlobal {
	pub ceremony_phase: CeremonyPhaseType,
	pub ceremony_index: CeremonyIndexType,
}

#[derive(Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AggregatedAccountData<AccountId, Moment> {
	pub global: AggregatedAccountDataGlobal,
	pub personal: Option<AggregatedAccountDataPersonal<AccountId, Moment>>,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Default,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct CeremonyInfo {
	pub ceremony_phase: CeremonyPhaseType,
	pub ceremony_index: CeremonyIndexType,
}

#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct ReputationCacheValue {
	pub ceremony_info: CeremonyInfo,
	pub reputation: Vec<(CeremonyIndexType, CommunityReputation)>,
}

#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	PartialEq,
	Eq,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum MeetupResult {
	Ok,
	VotesNotDependable,
	MeetupValidationIndexOutOfBounds,
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::Pair;
	use test_utils::{AccountId, AccountKeyring, Moment, Signature};

	#[test]
	fn claim_verification_works() {
		let alice = AccountKeyring::Alice.pair();
		let claim = ClaimOfAttendance::<Signature, AccountId, Moment>::new_unsigned(
			alice.public().into(),
			1,
			Default::default(),
			1,
			Default::default(),
			Default::default(),
			3,
		)
		.sign(&alice);

		assert!(claim.verify_signature())
	}

	#[test]
	fn proof_of_attendance_signing_works() {
		let alice = AccountKeyring::Alice.pair();

		let proof = ProofOfAttendance::<Signature, _>::signed(
			alice.public().into(),
			Default::default(),
			1,
			&alice,
		);

		assert!(proof.verify_signature())
	}
}
