use crate::{Pallet as ReputationCommitments, *};
use codec::Encode;
use encointer_primitives::{ceremonies::Reputation, reputation_commitments::FromStr};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::{BlakeTwo256, Hash};
use encointer_primitives::storage::participant_reputation;

benchmarks! {
	register_purpose {
		let zoran = account("zoran", 1, 1);
		let descriptor = DescriptorType::from_str("Some Descriptor").unwrap();
		assert!(<Purposes<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), descriptor)
	verify {
		assert!(<Purposes<T>>::iter().next().is_some());
	}


	commit_reputation {
		let zoran = account::<T::AccountId>("zoran", 1, 1);
		let cindex = 10;
		let cid = CommunityIdentifier::default();
		let descriptor = DescriptorType::from_str("Some Faucet Name").unwrap();
		ReputationCommitments::<T>::register_purpose(RawOrigin::Signed(zoran.clone()).into(), descriptor).ok();

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, cindex), &zoran), &Reputation::VerifiedUnlinked.encode());

		let hash = "Some value".using_encoded(BlakeTwo256::hash);

		assert!(<Commitments<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), cid, cindex, 0, Some(hash))
	verify {
		assert!(<Commitments<T>>::iter().next().is_some());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
