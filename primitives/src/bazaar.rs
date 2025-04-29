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

use parity_scale_codec::{Decode, DecodeWithMemTracking,Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

use crate::{common::PalletString, communities::CommunityIdentifier};

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, DecodeWithMemTracking,Default, RuntimeDebug, Clone, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct BusinessIdentifier<AccountId> {
	/// Community identifier this business operates in.
	pub community_identifier: CommunityIdentifier,
	/// Controller account that has the power to manage a business and its offerings.
	pub controller: AccountId,
}

impl<AccountId> BusinessIdentifier<AccountId> {
	pub fn new(cid: CommunityIdentifier, controller: AccountId) -> Self {
		Self { community_identifier: cid, controller }
	}
}

/// Data structure used for organizing the pallet's storage.
#[derive(Encode, Decode, DecodeWithMemTracking,Default, RuntimeDebug, Clone, Eq, PartialEq, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct BusinessData {
	/// Ipfs hash of the business metadata.
	pub url: PalletString,
	/// Strict-monotonic counter of the ID of the last offering.
	pub last_oid: u32,
}

impl BusinessData {
	pub fn new(url: PalletString, last_oid: u32) -> Self {
		Self { url, last_oid }
	}
}

/// Structure that contains the [BusinessData] including its controller account.
///
/// Intended as a return value for RPC requests.
#[derive(Encode, Decode, DecodeWithMemTracking,Default, RuntimeDebug, Clone, Eq, PartialEq, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Business<AccountId> {
	controller: AccountId,
	business_data: BusinessData,
}

impl<AccountId> Business<AccountId> {
	pub fn new(controller: AccountId, business_data: BusinessData) -> Self {
		Self { controller, business_data }
	}
}

pub type OfferingIdentifier = u32;

#[derive(Encode, Decode, DecodeWithMemTracking,Default, RuntimeDebug, Clone, Eq, PartialEq, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct OfferingData {
	pub url: PalletString,
}

impl OfferingData {
	pub fn new(url: PalletString) -> Self {
		Self { url }
	}
}
