use crate::{Pallet as EncointerDemocracy, *};
use encointer_primitives::{
    ceremonies::Reputation,
    communities::CommunityIdentifier,
    democracy::{ProposalState, Tally, Vote},
    storage::{current_ceremony_index_key, global_reputation_count, participant_reputation},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_support::{
    assert_ok,
    traits::{OnFinalize, OriginTrait},
    BoundedVec,
};
use frame_system::RawOrigin;
use parity_scale_codec::Encode;
#[cfg(not(feature = "std"))]
use sp_std::vec;

fn advance_timestamp_equivalent_to_n_blocks<T: Config>(n: u64) {
    let offset: T::Moment = (n * 6000u64)
        .try_into()
        .unwrap_or_else(|_| panic!("Something went horribly wrong!"));
    let new_time: T::Moment = pallet_timestamp::Pallet::<T>::get() + offset;
    let _ = pallet_timestamp::Pallet::<T>::set(T::RuntimeOrigin::none(), new_time);
    pallet_timestamp::Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
}

benchmarks! {
	where_clause {
		where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
		T::AccountId: AsRef<[u8; 32]>,
	}
	submit_proposal {
		let zoran = account("zoran", 1, 1);
		let cid = CommunityIdentifier::default();
		// worst case is petition
		let proposal_action = ProposalAction::Petition(Some(cid), PalletString::try_from("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\
		xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".as_bytes().to_vec()).unwrap());
		assert!(<Proposals<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), proposal_action)
	verify {
		assert!(<Proposals<T>>::iter().next().is_some());
	}

	vote {
		frame_support::storage::unhashed::put_raw(&current_ceremony_index_key(), &7u32.encode());

		let zoran = account::<T::AccountId>("zoran", 1, 1);
		let cid = CommunityIdentifier::default();

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 3), &zoran), &Reputation::VerifiedUnlinked.encode());
		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 4), &zoran), &Reputation::VerifiedUnlinked.encode());
		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, 5), &zoran), &Reputation::VerifiedUnlinked.encode());
		frame_support::storage::unhashed::put_raw(&global_reputation_count(3), &1u128.encode());
		frame_support::storage::unhashed::put_raw(&global_reputation_count(4), &1u128.encode());
		frame_support::storage::unhashed::put_raw(&global_reputation_count(5), &1u128.encode());

		let reputation_vec: ReputationVecOf<T> = BoundedVec::try_from(vec![
			(cid, 3),
			(cid, 4),
			(cid, 5),
		]).unwrap();

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::<T>::submit_proposal(
			RawOrigin::Signed(zoran.clone()).into(),
			proposal_action
		));

		assert_eq!(<Tallies<T>>::get(1).unwrap().ayes, 0);
	}: _(RawOrigin::Signed(zoran.clone()),
	1,
	Vote::Aye,
	reputation_vec)
	verify {
		assert_eq!(<Tallies<T>>::get(1).unwrap().ayes, 3);
	}

	update_proposal_state {
		frame_support::storage::unhashed::put_raw(&current_ceremony_index_key(), &7u32.encode());
		let zoran = account::<T::AccountId>("zoran", 1, 1);
		let cid = CommunityIdentifier::default();

		frame_support::storage::unhashed::put_raw(&global_reputation_count(5), &3u128.encode());

		let proposal_action = ProposalAction::SetInactivityTimeout(8);
		assert_ok!(EncointerDemocracy::<T>::submit_proposal(
			RawOrigin::Signed(zoran.clone()).into(),
			proposal_action
		));


		Tallies::<T>::insert(1, Tally { turnout: 3, ayes: 3 });

		assert_eq!(EncointerDemocracy::<T>::proposals(1).unwrap().state, ProposalState::Ongoing);
		EncointerDemocracy::<T>::update_proposal_state(RawOrigin::Signed(zoran.clone()).into(), 1).ok();
		assert!(<EnactmentQueue<T>>::iter().next().is_none());
		advance_timestamp_equivalent_to_n_blocks::<T>(21);

	}: _(RawOrigin::Signed(zoran), 1)
	verify {
		assert!(<EnactmentQueue<T>>::iter().next().is_some());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
