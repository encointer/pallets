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
	traits::{Get, OnTimestampSet},
	weights::DispatchClass,
};
use log::{info, warn};
use sp_runtime::traits::{CheckedDiv, One, Saturating, Zero};
use sp_std::{ops::Rem, prelude::*};

// Logger target
const LOG: &str = "encointer";

pub use crate::weights::WeightInfo;
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

		/// Required origin to interfere with the scheduling (though can always be Root)
		type CeremonyMaster: EnsureOrigin<Self::Origin>;

		/// Who to inform about ceremony phase change
		type OnCeremonyPhaseChange: OnCeremonyPhaseChange;
		#[pallet::constant]
		type MomentsPerDay: Get<Self::Moment>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event {
		/// Phase changed to `[new phase]`
		PhaseChangedTo(CeremonyPhaseType),
		CeremonySchedulePushedByOneDay,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// a division by zero occured
		DivisionByZero,
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
		CeremonyPhaseType::Registering
	}

	#[pallet::storage]
	#[pallet::getter(fn current_phase)]
	pub(super) type CurrentPhase<T: Config> =
		StorageValue<_, CeremonyPhaseType, ValueQuery, DefaultForCurrentPhase>;

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
				current_phase: CeremonyPhaseType::Registering,
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

			self.phase_durations.iter().for_each(|(k, v)| {
				<PhaseDurations<T>>::insert(k, v);
			});
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Manually transition to next phase without affecting the ceremony rhythm
		///
		/// May only be called from `T::CeremonyMaster`.
		#[pallet::weight((<T as Config>::WeightInfo::next_phase(), DispatchClass::Normal, Pays::Yes))]
		pub fn next_phase(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			T::CeremonyMaster::ensure_origin(origin)?;

			Self::progress_phase()?;

			Ok(().into())
		}

		/// Push next phase change by one entire day
		///
		/// May only be called from `T::CeremonyMaster`.
		#[pallet::weight((<T as Config>::WeightInfo::push_by_one_day(), DispatchClass::Normal, Pays::Yes))]
		pub fn push_by_one_day(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			T::CeremonyMaster::ensure_origin(origin)?;

			let tnext = Self::next_phase_timestamp().saturating_add(T::MomentsPerDay::get());
			<NextPhaseTimestamp<T>>::put(tnext);
			Self::deposit_event(Event::CeremonySchedulePushedByOneDay);
			Ok(().into())
		}

		#[pallet::weight((<T as Config>::WeightInfo::set_phase_duration(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_phase_duration(
			origin: OriginFor<T>,
			ceremony_phase: CeremonyPhaseType,
			duration: T::Moment,
		) -> DispatchResultWithPostInfo {
			T::CeremonyMaster::ensure_origin(origin)?;
			<PhaseDurations<T>>::insert(ceremony_phase, duration);
			Ok(().into())
		}

		#[pallet::weight((<T as Config>::WeightInfo::set_next_phase_timestamp(), DispatchClass::Normal, Pays::Yes))]
		pub fn set_next_phase_timestamp(
			origin: OriginFor<T>,
			timestamp: T::Moment,
		) -> DispatchResultWithPostInfo {
			T::CeremonyMaster::ensure_origin(origin)?;
			<NextPhaseTimestamp<T>>::put(timestamp);
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
			CeremonyPhaseType::Registering => CeremonyPhaseType::Assigning,
			CeremonyPhaseType::Assigning => CeremonyPhaseType::Attesting,
			CeremonyPhaseType::Attesting => {
				let next_ceremony_index = current_ceremony_index.saturating_add(1);
				<CurrentCeremonyIndex<T>>::put(next_ceremony_index);
				info!(target: LOG, "new ceremony phase with index {}", next_ceremony_index);
				CeremonyPhaseType::Registering
			},
		};

		let next = last_phase_timestamp.saturating_add(<PhaseDurations<T>>::get(next_phase));
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
		let cycle_duration = <PhaseDurations<T>>::get(CeremonyPhaseType::Registering) +
			<PhaseDurations<T>>::get(CeremonyPhaseType::Assigning) +
			<PhaseDurations<T>>::get(CeremonyPhaseType::Attesting);
		let now = <pallet_timestamp::Pallet<T>>::now();

		let tnext = if tnext < now {
			let gap = now - tnext;
			if let Some(n) = gap.checked_div(&cycle_duration) {
				tnext.saturating_add((cycle_duration).saturating_mul(n + T::Moment::one()))
			} else {
				return Err(<Error<T>>::DivisionByZero.into())
			}
		} else {
			let gap = tnext - now;
			if let Some(n) = gap.checked_div(&cycle_duration) {
				tnext.saturating_sub(cycle_duration.saturating_mul(n))
			} else {
				return Err(<Error<T>>::DivisionByZero.into())
			}
		};
		<NextPhaseTimestamp<T>>::put(tnext);
		info!(target: LOG, "next phase change at: {:?}", tnext);
		Ok(())
	}

	fn on_timestamp_set(now: T::Moment) {
		if Self::next_phase_timestamp() == T::Moment::zero() {
			// only executed in first block after genesis.

			// in case we upgrade from a runtime that didn't have this pallet or other curiosities
			if <CurrentCeremonyIndex<T>>::get() == 0 {
				<CurrentCeremonyIndex<T>>::put(1);
			}

			// set phase start to 0:00 UTC on the day of genesis
			let next = (now - now.rem(T::MomentsPerDay::get()))
				.saturating_add(<PhaseDurations<T>>::get(CeremonyPhaseType::Registering));

			if Self::resync_and_set_next_phase_timestamp(next).is_err() {
				warn!(target: LOG, "resync ceremony phase failed");
			};
		} else if Self::next_phase_timestamp() < now {
			if Self::progress_phase().is_err() {
				warn!(target: LOG, "progress ceremony phase failed");
			};
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

mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
