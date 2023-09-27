use crate::{Pallet as ReputationCommitments, *};
use codec::Encode;
use encointer_primitives::{
	ceremonies::Reputation,
	vouches::{PresenceType, Vouch, VouchQuality, VouchType},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::{BlakeTwo256, Hash};

benchmarks! {
	vouch_for {
		let zoran = account("zoran", 1, 1);
		let goran = account("goran", 1, 2);
		let vouch_type = VouchType::EncounteredHuman(PresenceType::Physical);
		let qualities = vec![VouchQuality::default(), 32];
		assert!(<Vouches<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), goran, vouch_type, qualities)
	verify {
		assert!(<Vouches<T>>::iter().next().is_some());
	}

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
