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

use support::{
    decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    storage::StorageValue,
    traits::Get,
    weights::{DispatchClass, Pays}
};
use system::ensure_signed;
use sp_timestamp::OnTimestampSet;
use rstd::prelude::*;
use runtime_io::misc::{print_utf8, print_hex};
use codec::{Decode, Encode};
use sp_runtime::traits::{CheckedAdd, Zero};
use rstd::ops::Rem;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub trait Trait: system::Trait  + timestamp::Trait
{
    type Event: From<Event> + Into<<Self as system::Trait>::Event>;
    type OnCeremonyPhaseChange: OnCeremonyPhaseChange;
    type MomentsPerDay: Get<Self::Moment>;
}

pub type CeremonyIndexType = u32;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CeremonyPhaseType {
    REGISTERING,
    ASSIGNING,
    ATTESTING,
}

impl Default for CeremonyPhaseType {
    fn default() -> Self {
        CeremonyPhaseType::REGISTERING
    }
}

/// An event handler for when the ceremony phase changes.
pub trait OnCeremonyPhaseChange {
	fn on_ceremony_phase_change(
		new_phase: CeremonyPhaseType,
	);
}

impl OnCeremonyPhaseChange for () {
    fn on_ceremony_phase_change(_: CeremonyPhaseType) { () }
}

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as EncointerScheduler {
        // caution: index starts with 1, not 0! (because null and 0 is the same for state storage)
        CurrentCeremonyIndex get(fn current_ceremony_index) config(): CeremonyIndexType;
        LastCeremonyBlock get(fn last_ceremony_block): T::BlockNumber;
        CurrentPhase get(fn current_phase) config(): CeremonyPhaseType = CeremonyPhaseType::REGISTERING;
        CeremonyMaster get(fn ceremony_master) config(): T::AccountId;
        NextPhaseTimestamp get(fn next_phase_timestamp): T::Moment = T::Moment::from(0);
        PhaseDurations get(fn phase_durations) config(): map hasher(blake2_128_concat) CeremonyPhaseType => T::Moment;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        const MomentsPerDay: T::Moment = T::MomentsPerDay::get();

        fn deposit_event() = default;

        #[weight = (1000, DispatchClass::Operational, Pays::No)]
        pub fn next_phase(origin) -> DispatchResult {
            let sender = ensure_signed(origin)?;
            ensure!(sender == <CeremonyMaster<T>>::get(), "only the CeremonyMaster can call this function");
            Self::progress_phase()?;
            Ok(())
        }
    }
}

decl_event!(
    pub enum Event {
        PhaseChangedTo(CeremonyPhaseType),
    }
);

impl<T: Trait> Module<T> {
    // implicitly assuming Moment to be unix epoch!
  
    fn progress_phase() -> DispatchResult {
        let current_phase = <CurrentPhase>::get();
        let current_ceremony_index = <CurrentCeremonyIndex>::get();
        
        let last_phase_timestamp = Self::next_phase_timestamp();

        let next_phase = match current_phase {
            CeremonyPhaseType::REGISTERING => {
                    CeremonyPhaseType::ASSIGNING
            },
            CeremonyPhaseType::ASSIGNING => {
                    CeremonyPhaseType::ATTESTING
            },
            CeremonyPhaseType::ATTESTING => {
                    let next_ceremony_index = match current_ceremony_index.checked_add(1) {
                        Some(v) => v,
                        None => 0, //deliberate wraparound
                    };
                    <CurrentCeremonyIndex>::put(next_ceremony_index);
                    CeremonyPhaseType::REGISTERING
            },
        };

        let next = last_phase_timestamp
            .checked_add(&<PhaseDurations<T>>::get(next_phase))
            .expect("overflowing timestamp");
        <NextPhaseTimestamp<T>>::put(next);

        <CurrentPhase>::put(next_phase);
        T::OnCeremonyPhaseChange::on_ceremony_phase_change(next_phase);
        Self::deposit_event(Event::PhaseChangedTo(next_phase));
        print_utf8(b"phase changed to:");
        print_hex(&next_phase.encode());
        Ok(())

    }

	fn on_timestamp_set(now: T::Moment) {
        if Self::next_phase_timestamp() == T::Moment::zero() {
            // only executed in first block after genesis. 
            // set phase start to 0:00 UTC on the day of genesis
            let next = (now - now.rem(T::MomentsPerDay::get()))
                .checked_add(&<PhaseDurations<T>>::get(CeremonyPhaseType::REGISTERING))
                .expect("overflowing timestamp");
            <NextPhaseTimestamp<T>>::put(next);
        } else if Self::next_phase_timestamp() < now {
            Self::progress_phase().expect("phase progress error");
        }
    }
}

impl<T: Trait> OnTimestampSet<T::Moment> for Module<T> {
	fn on_timestamp_set(moment: T::Moment) {
		Self::on_timestamp_set(moment)
	}
}

#[cfg(test)]
mod tests;
