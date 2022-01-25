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
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	ensure,
	storage::StorageValue,
	traits::{Get, OnTimestampSet},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use log::info;
use sp_runtime::traits::{CheckedAdd, CheckedDiv, One, Saturating, Zero};
use sp_std::{ops::Rem, prelude::*};

// Logger target
const LOG: &str = "encointer";

pub trait Config: frame_system::Config + pallet_timestamp::Config {
	type Event: From<Event> + Into<<Self as frame_system::Config>::Event>;
	type OnCeremonyPhaseChange: OnCeremonyPhaseChange;
	type MomentsPerDay: Get<Self::Moment>;
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		const MomentsPerDay: T::Moment = T::MomentsPerDay::get();

		fn deposit_event() = default;

		/// Manually transition to next phase without affecting the ceremony rhythm
		#[weight = (1000, DispatchClass::Operational, Pays::No)]
		pub fn next_phase(origin) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(sender == <CeremonyMaster<T>>::get(), Error::<T>::AuthorizationRequired);
			Self::progress_phase()?;
			Ok(())
		}

		/// Push next phase change by one entire day
		#[weight = (1000, DispatchClass::Operational, Pays::No)]
		pub fn push_by_one_day(origin) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			ensure!(sender == <CeremonyMaster<T>>::get(), Error::<T>::AuthorizationRequired);
			let tnext = Self::next_phase_timestamp().saturating_add(T::MomentsPerDay::get());
			<NextPhaseTimestamp<T>>::put(tnext);
			Ok(())
		}
	}
}

decl_event!(
	pub enum Event {
		PhaseChangedTo(CeremonyPhaseType),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
		/// sender doesn't have the necessary authority to perform action
		AuthorizationRequired,
	}
}

// This module's storage items.
decl_storage! {
	trait Store for Module<T: Config> as EncointerScheduler {
		// caution: index starts with 1, not 0! (because null and 0 is the same for state storage)
		CurrentCeremonyIndex get(fn current_ceremony_index) config(): CeremonyIndexType;
		LastCeremonyBlock get(fn last_ceremony_block): T::BlockNumber;
		CurrentPhase get(fn current_phase) config(): CeremonyPhaseType = CeremonyPhaseType::REGISTERING;
		CeremonyMaster get(fn ceremony_master) config(): T::AccountId;
		NextPhaseTimestamp get(fn next_phase_timestamp): T::Moment = T::Moment::zero();
		PhaseDurations get(fn phase_durations) config(): map hasher(blake2_128_concat) CeremonyPhaseType => T::Moment;
	}
}

impl<T: Config> Module<T> {
	// implicitly assuming Moment to be unix epoch!

	fn progress_phase() -> DispatchResult {
		let current_phase = <CurrentPhase>::get();
		let current_ceremony_index = <CurrentCeremonyIndex>::get();

		let last_phase_timestamp = Self::next_phase_timestamp();

		let next_phase = match current_phase {
			CeremonyPhaseType::REGISTERING => CeremonyPhaseType::ASSIGNING,
			CeremonyPhaseType::ASSIGNING => CeremonyPhaseType::ATTESTING,
			CeremonyPhaseType::ATTESTING => {
				let next_ceremony_index = current_ceremony_index.saturating_add(1);
				<CurrentCeremonyIndex>::put(next_ceremony_index);
				CeremonyPhaseType::REGISTERING
			},
		};

		let next = last_phase_timestamp
			.checked_add(&<PhaseDurations<T>>::get(next_phase))
			.expect("overflowing timestamp");
		Self::resync_and_set_next_phase_timestamp(next)?;

		<CurrentPhase>::put(next_phase);
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

impl<T: Config> OnTimestampSet<T::Moment> for Module<T> {
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
