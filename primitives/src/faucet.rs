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

use crate::{communities::CommunityIdentifier, reputation_commitments::PurposeIdType};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::{bounded::BoundedVec, ConstU32, MaxEncodedLen, RuntimeDebug};

#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

pub type WhiteListType = BoundedVec<CommunityIdentifier, ConstU32<1024>>;
pub type FaucetNameType = BoundedVec<u8, ConstU32<64>>;
use scale_info::prelude::vec::Vec;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, Default, MaxEncodedLen, TypeInfo)]
pub struct Faucet<AccountId, Balance> {
	pub name: FaucetNameType,
	pub purpose_id: PurposeIdType,
	pub whitelist: WhiteListType,
	pub drip_amount: Balance,
	pub creator: AccountId,
}

pub trait FromStr: Sized {
	type Err;
	fn from_str(inp: &str) -> Result<Self, Self::Err>;
}
impl FromStr for FaucetNameType {
	type Err = Vec<u8>;
	fn from_str(inp: &str) -> Result<Self, Self::Err> {
		Self::try_from(inp.as_bytes().to_vec())
	}
}
