// Copyright (c) 2023 Encointer Association
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

use crate::common::BoundedIpfsCid;
use parity_scale_codec::{Decode, DecodeWithMemTracking,Encode, MaxEncodedLen};
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

/// Did the attester meet the attestee physically, virtually or through asynchronous messages?
#[derive(Default, Encode, Decode, DecodeWithMemTracking,Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum PresenceType {
	/// could be "I have exchanged messages with the person I vouch for"
	/// could be "I have watched the replay of the Event or talk I vouch for"
	#[default]
	Asynchronous,
	/// could be "I have attended that online conference remotely"
	/// could be "I have visited that place in the metaverse"
	/// could be "I have met this person on an video call and they presented this account to me"
	LiveVirtual,
	/// could be "I met the human I vouch for in-person and scanned the account they presented at
	/// the occasion of this physical encounter" could be "I was standing in front of this monument
	/// and scanned the QR code on its plate" could be "I ate at this restaurant and scanned the QR
	/// code presented at their entrance in order to submit a rating"
	LivePhysical,
}

/// The nature of a vouch
/// this set is most likely incomplete. we leave gaps in the encoding to have room for more kinds
/// which still results in meaningful order
#[derive(Default, Encode, Decode, DecodeWithMemTracking,Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum VouchKind {
	/// Unspecified. This should generally be handeled as an invalid vouch or an alien use case
	#[default]
	Unspecified,
	/// This person is know to me and I have verified their account with specified presence type
	KnownHuman(PresenceType),
	/// I do not claim to know this person, but I encountered a human being providing me with the
	/// account I vouch for
	EncounteredHuman(PresenceType),
	/// I encountered an object showing the account I vouch for
	EncounteredObject(PresenceType),
	/// I have visited a place labeled with the account I vouch for
	VisitedPlace(PresenceType),
	/// I have attended an event which identifies with the account I vouch for
	AttendedEvent(PresenceType),
}

/// a scalar expression of quality. Interpretation left to client side per use case
/// could be a 0-5 star rating or a high-resolution 0..255
pub type Rating = u8;

/// additional information about the attestee's qualities
#[derive(Default, Encode, Decode, DecodeWithMemTracking,Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum VouchQuality {
	/// Don't want to submit additional information
	#[default]
	Unspecified,
	/// a generic badge for qualitative attestation stored as a json file on IPFS (json schema TBD)
	Badge(BoundedIpfsCid),
	/// a quantitative expression of the attestee's quality (i.e. number of stars)
	Rating(Rating),
}

#[derive(Default, Encode, Decode, DecodeWithMemTracking,PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Vouch<Moment> {
	/// protected vouches can't be purged. unprotected ones can be lazily purged after a
	/// time-to-live. (future feature)
	pub protected: bool,
	/// the timestamp of the block which registers this Vouch
	pub timestamp: Moment,
	/// what is the nature of this vouch?
	pub vouch_kind: VouchKind,
	/// additional information about the attestee's qualities
	pub quality: VouchQuality,
}
