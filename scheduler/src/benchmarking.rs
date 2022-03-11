#![cfg(any(test, feature = "runtime-benchmarks"))]

use super::*;

use crate::Pallet as Scheduler;
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::assert_ok;
use frame_system::RawOrigin;

benchmarks! {
	next_phase {
		crate::CurrentPhase::<T>::put(CeremonyPhaseType::ATTESTING);
	}: {
		assert_ok!(Scheduler::<T>::next_phase(RawOrigin::Root.into()));
	}
	verify {
	}

	push_by_one_day {
	}: {
		assert_ok!(Scheduler::<T>::push_by_one_day(RawOrigin::Root.into()));
	}
	verify {
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(86_400_000), crate::mock::TestRuntime);
