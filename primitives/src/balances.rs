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
use ep_core::fixed::types::I64F64;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};

// We're working with fixpoint here.
pub type BalanceType = I64F64;
pub type Demurrage = I64F64;

#[derive(Encode, Decode, Default, RuntimeDebug, Clone, Copy, TypeInfo)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
pub struct BalanceEntry<BlockNumber> {
	/// The balance of the account after last manual adjustment
	pub principal: BalanceType,
	/// The time (block height) at which the balance was last adjusted
	pub last_update: BlockNumber,
}

pub mod consts {
	pub const DEFAULT_DEMURRAGE: i128 = 0x0000000000000000000001E3F0A8A973_i128;
}
