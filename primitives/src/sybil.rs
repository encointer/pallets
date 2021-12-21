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
use ep_core::fixed::traits::Fixed;
use scale_info::TypeInfo;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::{BlakeTwo256, Hash};
use sp_std::vec::Vec;
use xcm::{
	opaque::v1::{Junction::Parachain, MultiLocation},
	prelude::X1,
};

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

use crate::{
	scheduler::CeremonyIndexType, sybil::consts::ISSUE_PERSONHOOD_UNIQUENESS_RATING_WEIGHT,
};

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct IssuePersonhoodUniquenessRatingCall {
	call_index: [u8; 2],
	request: OpaqueRequest,
	response_meta: CallMetadata,
}

pub type OpaqueRequest = Vec<u8>;

pub trait RequestHash {
	fn hash(&self) -> H256;
}

impl RequestHash for OpaqueRequest {
	fn hash(&self) -> H256 {
		self.using_encoded(BlakeTwo256::hash)
	}
}

impl IssuePersonhoodUniquenessRatingCall {
	pub fn new(
		personhood_oracle_index: u8,
		request: OpaqueRequest,
		response_meta: CallMetadata,
	) -> Self {
		Self {
			call_index: [personhood_oracle_index, 0], // is the first call in personhood-oracle pallet
			request,
			response_meta,
		}
	}

	pub fn request_hash(&self) -> H256 {
		self.request.hash()
	}

	/// Returns the (currently hardcoded) weight of the `issue_personhood_uniqueness_rating` call
	pub fn weight(&self) -> u64 {
		ISSUE_PERSONHOOD_UNIQUENESS_RATING_WEIGHT
	}
}

/// Contains the necessary information to ask for an XCM return message.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, Default, Copy, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct CallMetadata {
	/// The index must match the position of the module in `construct_runtime!`.
	pallet_index: u8,
	/// The index must match the position of the dispatchable in the target pallet.
	call_index: u8,
	/// The weight of `call`; this should be at least the chain's calculated weight.
	require_weight_at_most: u64,
}

impl CallMetadata {
	pub fn new(pallet_index: u8, call_index: u8, require_weight_at_most: u64) -> Self {
		return Self { pallet_index, call_index, require_weight_at_most }
	}

	pub fn weight(&self) -> u64 {
		self.require_weight_at_most
	}
}

/// This allows to generically call the sybil-personhood-oracle, whose response calls the method with the
/// index defined in the `SybilResponse`
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub enum SybilResponse {
	Faucet = 1,
}

impl Default for SybilResponse {
	fn default() -> SybilResponse {
		SybilResponse::Faucet
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct SybilResponseCall {
	call_index: [u8; 2],
	request_hash: H256,
	confidence: PersonhoodUniquenessRating,
	#[codec(skip)]
	xcm_weight: u64,
}

impl SybilResponseCall {
	pub fn new(
		response: &CallMetadata,
		request_hash: H256,
		confidence: PersonhoodUniquenessRating,
	) -> Self {
		Self {
			call_index: [response.pallet_index, response.call_index],
			request_hash,
			confidence,
			xcm_weight: response.require_weight_at_most,
		}
	}

	pub fn weight(&self) -> u64 {
		self.xcm_weight
	}
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct PersonhoodUniquenessRating {
	attested: CeremonyIndexType,
	last_n_ceremonies: CeremonyIndexType,
	proofs: Vec<H256>,
}

impl PersonhoodUniquenessRating {
	pub fn new(
		attested: CeremonyIndexType,
		last_n_ceremonies: CeremonyIndexType,
		proofs: Vec<H256>,
	) -> Self {
		Self { attested, last_n_ceremonies, proofs }
	}

	pub fn proofs(&self) -> Vec<H256> {
		self.proofs.clone()
	}

	pub fn as_ratio<F: Fixed>(&self) -> F {
		return F::from_num(self.attested)
			.checked_div(F::from_num(self.last_n_ceremonies))
			.unwrap_or_default()
	}
}

pub fn sibling_junction(id: u32) -> MultiLocation {
	MultiLocation { parents: 1, interior: X1(Parachain(id)) }
}

pub mod consts {
	pub const SYBIL_CALL_WEIGHT: u64 = 5_000_000;
	pub const ISSUE_PERSONHOOD_UNIQUENESS_RATING_WEIGHT: u64 = 5_000_000;
}
