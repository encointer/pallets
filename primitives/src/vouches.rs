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
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

/// Did the attester meet the attestee physically, virtually or through asynchronous messages?
#[derive(Default, Encode, Decode, Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum PresenceType {
	#[default]
	Asynchronous,
	Virtual,
	Physical,
}

/// The nature of a vouch
/// this set is most likely incomplete. we leave gaps in the encoding to have room for more kinds
/// which still results in meaningful order
#[derive(Default, Encode, Decode, Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum VouchKind {
	#[default]
	Unspecified = 0,
	KnownHuman = 10u8,
	EncounteredHuman(PresenceType) = 20u8,
	EncounteredObject(PresenceType) = 30u8,
	VisitedEvent(PresenceType) = 40u8,
	VisitedPlace(PresenceType) = 50u8,
}

/// a scalar expression of quality. Interpretation left to client side per use case
/// could be a 0-5 star rating or a high-resolution 0..255
pub type Rating = u8;

/// additional information about the attestee's qualities
#[derive(Default, Encode, Decode, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum VouchQuality {
	#[default]
	Unspecified,
	/// a generic badge for qualitative attestation stored as a json file on IPFS (json schema TBD)
	Badge(BoundedIpfsCid),
	/// a quantitative expression of the attestee's quality (i.e. number of stars)
	Rating(Rating),
}

#[derive(Default, Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct Vouch<Moment> {
	/// protected vouches can't be purged. unprotected ones can be lazily purged after a time-to-live. (future feature)
	pub protected: bool,
	/// the timestamp of the block which registers this Vouch
	pub timestamp: Moment,
	/// what is the nature of this vouch?
	pub vouch_kind: VouchKind,
	/// additional information about the attestee's qualities
	pub quality: VouchQuality,
}
