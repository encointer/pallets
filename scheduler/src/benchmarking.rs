#![cfg(any(test, feature = "runtime-benchmarks"))]

use super::*;

use crate::Pallet as Scheduler;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
	next_phase {
		crate::CurrentPhase::<T>::put(CeremonyPhaseType::ATTESTING);
	}: _(RawOrigin::Root)
	verify {
		assert_eq!(crate::CurrentPhase::<T>::get(), CeremonyPhaseType::REGISTERING)
	}

	push_by_one_day {
		let current_timestamp = crate::NextPhaseTimestamp::<T>::get();
	}: _(RawOrigin::Root)
	verify {
		assert_eq!(crate::NextPhaseTimestamp::<T>::get(), current_timestamp + T::MomentsPerDay::get());
	}

	set_phase_duration {
		let timestamp: T::Moment = 1_000_000u32.into();
	}: _(RawOrigin::Root, CeremonyPhaseType::REGISTERING, timestamp)
	verify {
		assert_eq!(Scheduler::<T>::phase_durations(CeremonyPhaseType::REGISTERING), timestamp);
	}

	set_next_phase_timestamp {
		let timestamp: T::Moment = 1_000_000u32.into();
	}: _(RawOrigin::Root, timestamp)
	verify {
		assert_eq!(Scheduler::<T>::next_phase_timestamp(), timestamp);
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(86_400_000), crate::mock::TestRuntime);
