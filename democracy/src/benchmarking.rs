use crate::{Pallet as EncointerDemocracy, *};
use codec::Encode;
use encointer_primitives::{
	ceremonies::Reputation,
	communities::CommunityIdentifier,
	democracy::{ProposalState, Tally, Vote},
	storage::{current_ceremony_index_key, global_reputation_count, participant_reputation},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{assert_ok, traits::OnInitialize, BoundedVec};
use frame_system::RawOrigin;
#[cfg(not(feature = "std"))]
use sp_std::vec;

fn advance_n_blocks<T: Config>(n: u64) {
	for _ in 0..n {
		frame_system::Pallet::<T>::set_block_number(
			frame_system::Pallet::<T>::block_number() + 1u32.into(),
		);
		frame_system::Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
	}
}

benchmarks! {

	submit_proposal {
		let zoran = account("zoran", 1, 1);
		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert!(<Proposals<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), proposal_action)
	verify {
		assert!(<Proposals<T>>::iter().next().is_some());
	}

	vote {
		frame_support::storage::unhashed::put_raw(&current_ceremony_index_key(), &7u32.encode());

		let zoran = account::<T::AccountId>("zoran", 1, 1);
		let cid = CommunityIdentifier::default();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::<T>::submit_proposal(
			RawOrigin::Signed(zoran.clone()).into(),
			proposal_action
		));

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 3), &zoran), &Reputation::VerifiedUnlinked.encode());
		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 4), &zoran), &Reputation::VerifiedUnlinked.encode());
		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 5), &zoran), &Reputation::VerifiedUnlinked.encode());
		let reputation_vec: ReputationVecOf<T> = BoundedVec::try_from(vec![
			(cid, 3),
			(cid, 4),
			(cid, 5),
		]).unwrap();

		assert!(<VoteEntries<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran.clone()),
	1,
	Vote::Aye,
	reputation_vec)
	verify {
		assert!(<VoteEntries<T>>::iter().next().is_some());
	}

	update_proposal_state {
		frame_support::storage::unhashed::put_raw(&current_ceremony_index_key(), &7u32.encode());
		let zoran = account::<T::AccountId>("zoran", 1, 1);
		let cid = CommunityIdentifier::default();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::<T>::submit_proposal(
			RawOrigin::Signed(zoran.clone()).into(),
			proposal_action
		));


		Tallies::<T>::insert(1, Tally { turnout: 3, ayes: 3 });

		frame_support::storage::unhashed::put_raw(&global_reputation_count(5), &3u128.encode());
		assert_eq!(EncointerDemocracy::<T>::proposals(1).unwrap().state, ProposalState::Ongoing);
		EncointerDemocracy::<T>::update_proposal_state(RawOrigin::Signed(zoran.clone()).into(), 1).ok();
		assert!(<EnactmentQueue<T>>::iter().next().is_none());
		advance_n_blocks::<T>(21);

	}: _(RawOrigin::Signed(zoran), 1)
	verify {
		assert!(<EnactmentQueue<T>>::iter().next().is_some());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
