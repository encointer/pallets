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

//! # Encointer Scheduler Module
//!
//! provides functionality for
//! - scheduling ceremonies with their different phases
//! - dispatch transition functions upon phase change
//!

#![cfg_attr(not(feature = "std"), no_std)]

use encointer_primitives::scheduler::{CeremonyIndexType, CeremonyPhaseType};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{Get, OnTimestampSet},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use log::info;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, One, Saturating, Zero};
use sp_std::{ops::Rem, prelude::*};

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
	pub trait Config: frame_system::Config + pallet_timestamp::Config {
		type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;
		type OnCeremonyPhaseChange: OnCeremonyPhaseChange;
		#[pallet::constant]
		type MomentsPerDay: Get<Self::Moment>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event {
		/// Phase changed to `[new phase]`
		PhaseChangedTo(CeremonyPhaseType),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Sender doesn't have the necessary authority to perform action
		AuthorizationRequired,
	}

	#[pallet::storage]
	#[pallet::getter(fn current_ceremony_index)]
	pub(super) type CurrentCeremonyIndex<T: Config> =
		StorageValue<_, CeremonyIndexType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_ceremony_block)]
	pub(super) type LastCeremonyBlock<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	#[pallet::type_value]
	pub(super) fn DefaultForCurrentPhase() -> CeremonyPhaseType {
		CeremonyPhaseType::REGISTERING
	}

	#[pallet::storage]
	#[pallet::getter(fn current_phase)]
	pub(super) type CurrentPhase<T: Config> =
		StorageValue<_, CeremonyPhaseType, ValueQuery, DefaultForCurrentPhase>;

	#[pallet::storage]
	#[pallet::getter(fn ceremony_master)]
	pub(super) type CeremonyMaster<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

	#[pallet::type_value]
	pub(super) fn DefaultForNextPhaseTimestamp<T: Config>() -> T::Moment {
		T::Moment::zero()
	}

	#[pallet::storage]
	#[pallet::getter(fn next_phase_timestamp)]
	pub(super) type NextPhaseTimestamp<T: Config> =
		StorageValue<_, T::Moment, ValueQuery, DefaultForNextPhaseTimestamp<T>>;

	#[pallet::storage]
	#[pallet::getter(fn phase_durations)]
	pub(super) type PhaseDurations<T: Config> =
		StorageMap<_, Blake2_128Concat, CeremonyPhaseType, T::Moment, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		pub current_ceremony_index: CeremonyIndexType,
		pub current_phase: CeremonyPhaseType,
		pub ceremony_master: Option<T::AccountId>,
		pub phase_durations: Vec<(CeremonyPhaseType, T::Moment)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		fn default() -> Self {
			Self {
				current_ceremony_index: Default::default(),
				current_phase: CeremonyPhaseType::REGISTERING,
				ceremony_master: None,
				phase_durations: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T>
	where
		<T as pallet_timestamp::Config>::Moment: MaybeSerializeDeserialize,
	{
		fn build(&self) {
			<CurrentCeremonyIndex<T>>::put(&self.current_ceremony_index);
			<CurrentPhase<T>>::put(&self.current_phase);

			if let Some(ref ceremony_master) = self.ceremony_master {
				// First I thought, it might be sensible to put an expect here. However, one can always
				// edit the genesis config afterwards, so we can't really prevent here anything.
				//
				// substrate does the same in the sudo pallet.
				<CeremonyMaster<T>>::put(&ceremony_master);
			}

			self.phase_durations.iter().for_each(|(k, v)| {
				<PhaseDurations<T>>::insert(k, v);
			});
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Manually transition to next phase without affecting the ceremony rhythm
		#[pallet::weight((1000, DispatchClass::Operational, Pays::No))]
		pub fn next_phase(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let master = <CeremonyMaster<T>>::get().ok_or(Error::<T>::AuthorizationRequired)?;
			ensure!(sender == master, Error::<T>::AuthorizationRequired);

			Self::progress_phase()?;
			Ok(().into())
		}

		/// Push next phase change by one entire day
		#[pallet::weight((1000, DispatchClass::Operational, Pays::No))]
		pub fn push_by_one_day(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			let master = <CeremonyMaster<T>>::get().ok_or(Error::<T>::AuthorizationRequired)?;
			ensure!(sender == master, Error::<T>::AuthorizationRequired);

			let tnext = Self::next_phase_timestamp().saturating_add(T::MomentsPerDay::get());
			<NextPhaseTimestamp<T>>::put(tnext);
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	// implicitly assuming Moment to be unix epoch!

	fn progress_phase() -> DispatchResult {
		let current_phase = <CurrentPhase<T>>::get();
		let current_ceremony_index = <CurrentCeremonyIndex<T>>::get();

		let last_phase_timestamp = Self::next_phase_timestamp();

		let next_phase = match current_phase {
			CeremonyPhaseType::REGISTERING => CeremonyPhaseType::ASSIGNING,
			CeremonyPhaseType::ASSIGNING => CeremonyPhaseType::ATTESTING,
			CeremonyPhaseType::ATTESTING => {
				let next_ceremony_index = current_ceremony_index.saturating_add(1);
				<CurrentCeremonyIndex<T>>::put(next_ceremony_index);
				CeremonyPhaseType::REGISTERING
			},
		};

		let next = last_phase_timestamp
			.checked_add(&<PhaseDurations<T>>::get(next_phase))
			.expect("overflowing timestamp");
		Self::resync_and_set_next_phase_timestamp(next)?;

		<CurrentPhase<T>>::put(next_phase);
		T::OnCeremonyPhaseChange::on_ceremony_phase_change(next_phase);
		Self::deposit_event(Event::PhaseChangedTo(next_phase));
		info!(target: LOG, "phase changed to: {:?}", next_phase);
		Ok(())
	}

	// we need to resync in two situations:
	// 1. when the chain bootstraps and cycle duration is smaller than 24h, phases would cycle with every block until catched up
	// 2. when next_phase() is used, we would introduce long idle phases because next_phase_timestamp would be pushed furhter and further into the future
	fn resync_and_set_next_phase_timestamp(tnext: T::Moment) -> DispatchResult {
		let cycle_duration = <PhaseDurations<T>>::get(CeremonyPhaseType::REGISTERING) +
			<PhaseDurations<T>>::get(CeremonyPhaseType::ASSIGNING) +
			<PhaseDurations<T>>::get(CeremonyPhaseType::ATTESTING);
		let now = <pallet_timestamp::Pallet<T>>::now();

		let tnext = if tnext < now {
			let gap = now - tnext;
			let n = gap
				.checked_div(&cycle_duration)
				.expect("invalid phase durations: may not be zero");
			tnext.saturating_add((cycle_duration).saturating_mul(n + T::Moment::one()))
		} else {
			let gap = tnext - now;
			let n = gap
				.checked_div(&cycle_duration)
				.expect("invalid phase durations: may not be zero");
			tnext.saturating_sub(cycle_duration.saturating_mul(n))
		};
		<NextPhaseTimestamp<T>>::put(tnext);
		info!(target: LOG, "next phase change at: {:?}", tnext);
		Ok(())
	}

	fn on_timestamp_set(now: T::Moment) {
		if Self::next_phase_timestamp() == T::Moment::zero() {
			// only executed in first block after genesis.
			// set phase start to 0:00 UTC on the day of genesis
			let next = (now - now.rem(T::MomentsPerDay::get()))
				.checked_add(&<PhaseDurations<T>>::get(CeremonyPhaseType::REGISTERING))
				.expect("overflowing timestamp");
			Self::resync_and_set_next_phase_timestamp(next).expect("set next phase failed");
		} else if Self::next_phase_timestamp() < now {
			Self::progress_phase().expect("phase progress error");
		}
	}
}

impl<T: Config> OnTimestampSet<T::Moment> for Pallet<T> {
	fn on_timestamp_set(moment: T::Moment) {
		Self::on_timestamp_set(moment)
	}
}

/// An event handler for when the ceremony phase changes.
pub trait OnCeremonyPhaseChange {
	fn on_ceremony_phase_change(new_phase: CeremonyPhaseType);
}

impl OnCeremonyPhaseChange for () {
	fn on_ceremony_phase_change(_: CeremonyPhaseType) {}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
