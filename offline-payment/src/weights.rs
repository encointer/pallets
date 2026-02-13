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

use frame_support::weights::Weight;

pub trait WeightInfo {
	fn register_offline_identity() -> Weight;
	fn submit_offline_payment() -> Weight;
	fn submit_native_offline_payment() -> Weight;
	fn set_verification_key() -> Weight;
}

impl WeightInfo for () {
	fn register_offline_identity() -> Weight {
		Weight::from_parts(10_000, 0)
	}

	fn submit_offline_payment() -> Weight {
		// Higher due to ZK pairing operations
		Weight::from_parts(500_000_000, 0)
	}

	fn submit_native_offline_payment() -> Weight {
		// Same weight as CC — dominated by pairing ops
		Weight::from_parts(500_000_000, 0)
	}

	fn set_verification_key() -> Weight {
		// VK deserialization validation
		Weight::from_parts(100_000_000, 0)
	}
}
