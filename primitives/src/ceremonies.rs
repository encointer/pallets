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

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

use crate::communities::{CommunityIdentifier, Location};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::{BlakeTwo256, Hash, IdentifyAccount, Verify};

pub use crate::scheduler::CeremonyIndexType;

pub type ParticipantIndexType = u64;
pub type MeetupIndexType = u64;
pub type AttestationIndexType = u64;
pub type CommunityCeremony = (CommunityIdentifier, CeremonyIndexType);
pub type InactivityTimeoutType = u32;
pub type EndorsementTicketsPerBootstrapperType = u8;
pub type ReputationLifetimeType = u32;
pub type MeetupTimeOffsetType = i32;

#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
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

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
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
	Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
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

#[derive(
	Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo, MaxEncodedLen,
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

#[derive(
	Encode, Decode, Default, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen,
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
	Encode, Decode, Default, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Assignment {
	pub bootstrappers_reputables: AssignmentParams,
	pub endorsees: AssignmentParams,
	pub newbies: AssignmentParams,
	pub locations: AssignmentParams,
}

// Todo: abstract AssignmentParams trait and use two different structs: AssignmentParams, LocationAssignmentParams
#[derive(
	Encode, Decode, Default, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct AssignmentParams {
	/// Random prime below number of meetup participants. For locations this is the amount of locations.
	pub m: u64,
	/// First random group element in the interval (0, m). For locations this is a random coprime < m.
	pub s1: u64,
	/// Second random group element in the interval (0, m). For locations the closest prime to m,
	/// with s2 < m.
	pub s2: u64,
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
}
