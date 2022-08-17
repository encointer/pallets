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

//! # Encointer Democracy Module
//!

#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use encointer_primitives::proposals::Proposal;
use frame_support::{
	dispatch::DispatchResult,
	traits::{Get, OnTimestampSet},
	weights::DispatchClass,
};
use log::{info, warn};

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type ProposalId: Decode + Encode + MaxEncodedLen + EncodeLike + TypeInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		SomeEvent,
	}

	#[pallet::error]
	pub enum Error<T> {
		SomeError,
	}

	#[pallet::storage]
	#[pallet::getter(fn proposals)]
	pub(super) type CurrentCeremonyIndex<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ProposalId, Proposal<T::BlockNumber>, OptionQuery>;

	// #[pallet::genesis_config]
	// pub struct GenesisConfig<T: Config> {}
	//
	// #[cfg(feature = "std")]
	// impl<T: Config> Default for GenesisConfig<T> {
	// 	fn default() -> Self {
	// 		Self {}
	// 	}
	// }

	// #[pallet::genesis_build]
	// impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
	// 	fn build(&self) {}
	// }

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10000)]
		pub fn submit_proposal(
			origin: OriginFor<T>,
			proposal: Proposal,
		) -> DispatchResultWithPostInfo {
			Ok(().into())
		}
	}
	impl<T: Config> Pallet<T> {}
}

// mod weights;
//
// #[cfg(test)]
// mod mock;
// #[cfg(test)]
// mod tests;
//
// #[cfg(feature = "runtime-benchmarks")]
// mod benchmarking;
