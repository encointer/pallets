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
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;

use crate::common::PalletString;
use crate::communities::CommunityIdentifier;

#[derive(Encode, Decode, Default, RuntimeDebug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BusinessIdentifier<AccountId> {
    pub community_identifier: CommunityIdentifier,
    pub controller: AccountId,
}

impl<AccountId> BusinessIdentifier<AccountId> {
    pub fn new(cid: CommunityIdentifier, bid: AccountId) -> BusinessIdentifier<AccountId> {
        BusinessIdentifier {
            community_identifier: cid,
            controller: bid,
        }
    }
}

#[derive(Encode, Decode, Default, RuntimeDebug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct BusinessData {
    pub url: PalletString,
    pub last_oid: u32,
}

impl BusinessData {
    pub fn new(url: PalletString, last_oid: u32) -> BusinessData {
        return BusinessData { url, last_oid };
    }
}

pub type OfferingIdentifier = u32;

#[derive(Encode, Decode, Default, RuntimeDebug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct OfferingData {
    pub url: PalletString,
}

impl OfferingData {
    pub fn new(url: PalletString) -> OfferingData {
        return OfferingData { url };
    }
}