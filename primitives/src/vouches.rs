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
use frame_support::{
	traits::{ConstU32, Get},
	BoundedVec, RuntimeDebug,
};
use scale_info::TypeInfo;
#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

#[derive(Default, Encode, Decode, Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum PresenceType {
	#[default]
	Unspecified,
	Virtual,
	Physical,
}

#[derive(Default, Encode, Decode, Copy, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum VouchType {
	#[default]
	Unspecified = 0,
	KnownHuman = 10,
	EncounteredHuman(PresenceType) = 20,
	EncounteredObject(PresenceType) = 30,
	VisitedEvent(PresenceType) = 40,
	VisitedPlace(PresenceType) = 50,
}

pub type Rating = u8;

#[derive(Default, Encode, Decode, Clone, PartialEq, Eq, Debug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum VouchQuality {
	#[default]
	Unspecified,
	Badge(BoundedIpfsCid),
	Rating(Rating),
}

#[derive(Default, Encode, Decode, PartialEq, Eq, RuntimeDebug, Clone, TypeInfo, MaxEncodedLen)]
pub struct Vouch<Moment> {
	pub protected: bool,
	pub timestamp: Moment,
	pub vouch_type: VouchType,
	pub quality: VouchQuality,
}
