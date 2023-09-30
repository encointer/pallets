use crate::*;
use encointer_primitives::vouches::{PresenceType, VouchKind, VouchQuality};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;

benchmarks! {
	vouch_for {
		let zoran = account("zoran", 1, 1);
		let goran = account("goran", 1, 2);
		let vouch_kind = VouchKind::EncounteredHuman(PresenceType::LivePhysical);
		let quality = VouchQuality::default();
		assert!(<Vouches<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), goran, vouch_kind, quality)
	verify {
		assert!(<Vouches<T>>::iter().next().is_some());
	}

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
