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
use sp_core::{bounded::BoundedVec, ConstU32};

pub type PurposeIdType = u64;
pub type DescriptorType = BoundedVec<u8, ConstU32<24>>;

pub trait FromStr: Sized {
	type Err;
	fn from_str(inp: &str) -> Result<Self, Self::Err>;
}

impl FromStr for DescriptorType {
	type Err = Vec<u8>;
	fn from_str(inp: &str) -> Result<Self, Self::Err> {
		Self::try_from(inp.as_bytes().to_vec())
	}
}
