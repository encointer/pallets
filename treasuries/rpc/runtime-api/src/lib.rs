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

//! Runtime API definition required by Treasuries RPC extensions.

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_primitives::communities::CommunityIdentifier;
use parity_scale_codec::{Decode, DecodeWithMemTracking,Encode};

sp_api::decl_runtime_apis! {
	pub trait TreasuriesApi<AccountId>
	where AccountId: Encode + Decode
	{
		fn get_community_treasury_account_unchecked(maybecid: &Option<CommunityIdentifier>) -> AccountId;
	}
}
