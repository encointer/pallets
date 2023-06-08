use crate::{Pallet as FaucetPallet, *};
use codec::Encode;
use encointer_primitives::{
	ceremonies::Reputation,
	communities::{CommunityMetadata as CommunityMetadataType, Degree, Location},
	faucet::FromStr,
	storage::{community_identifiers, participant_reputation},
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, vec};
use frame_support::BoundedVec;
use frame_system::RawOrigin;

#[cfg(not(feature = "std"))]
use sp_std::vec::Vec;

pub type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

fn bootstrappers<T: frame_system::Config>() -> Vec<T::AccountId> {
	let alice: T::AccountId = account("alice", 1, 1);
	let bob: T::AccountId = account("bob", 2, 2);
	let charlie: T::AccountId = account("charlie", 3, 3);

	vec![alice.clone(), bob.clone(), charlie.clone()]
}

fn test_location() -> Location {
	Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) }
}

fn create_community<T: Config>() -> CommunityIdentifier {
	let location = test_location();
	let bs = bootstrappers::<T>();

	encointer_communities::Pallet::<T>::new_community(
		RawOrigin::Root.into(),
		location,
		bs.clone(),
		CommunityMetadataType::default(),
		None,
		None,
	)
	.ok();
	let cid = CommunityIdentifier::new(location, bs).unwrap();

	// somehow the cid is not persisted in the communities pallet, hence ht manual input
	let cids: Vec<CommunityIdentifier> = vec![cid];
	frame_support::storage::unhashed::put_raw(&community_identifiers(), &cids.encode());

	cid
}

benchmarks! {
	where_clause {
		where
		sp_core::H256: From<<T as frame_system::Config>::Hash>,
		T::AccountId: AsRef<[u8; 32]>,
		ReserveIdentifierOf<T>: From<[u8; 8]>,
	}
	create_faucet {
		let cid = create_community::<T>();
		let zoran = account("zoran", 1, 1);
		<T as pallet::Config>::Currency::make_free_balance_be(&zoran, 200_000_000u32.into());
		let faucet_name = FaucetNameType::from_str("Some Faucet Name").unwrap();
		let amount: BalanceOf<T> = 100_000_000u32.into();
		let drip_amount: BalanceOf<T> = 10_000u32.into();
		let whitelist = BoundedVec::try_from(vec![cid; 10]).unwrap();
		assert!(<Faucets<T>>::iter().next().is_none());
	}: _(RawOrigin::Signed(zoran), faucet_name, amount, whitelist, drip_amount)
	verify {
		assert!(<Faucets<T>>::iter().next().is_some());
	}


	drip {
		let cid = create_community::<T>();
		let cindex = 10;
		let zoran = account("zoran", 1, 1);
		let dripper = account::<T::AccountId>("dripper", 2, 2);

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, cindex), &dripper), &Reputation::VerifiedUnlinked.encode());
		<T as pallet::Config>::Currency::make_free_balance_be(&zoran, 200_000_000u32.into());
		let faucet_name = FaucetNameType::from_str("Some Faucet Name").unwrap();
		let amount: BalanceOf<T> = 100_000_000u32.into();
		let drip_amount: BalanceOf<T> = 10_000u32.into();
		let whitelist = BoundedVec::try_from(vec![cid; 10]).unwrap();
		assert!(<Faucets<T>>::iter().next().is_none());
		FaucetPallet::<T>::create_faucet(RawOrigin::Signed(zoran).into(), faucet_name, amount, whitelist, drip_amount).ok();
		let faucet_account = <Faucets<T>>::iter().next().unwrap().0;

	}: _(RawOrigin::Signed(dripper.clone()), faucet_account, cid, cindex)
	verify {
		assert_eq!(<T as pallet::Config>::Currency::free_balance(&dripper), 10_000u32.into());
	}

	dissolve_faucet {
		let cid = create_community::<T>();
		let cindex = 10;
		let zoran = account("zoran", 1, 1);
		let dripper = account::<T::AccountId>("dripper", 2, 2);
		let beneficiary = account::<T::AccountId>("beneficiary", 3, 3);

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, cindex), &dripper), &Reputation::VerifiedUnlinked.encode());
		<T as pallet::Config>::Currency::make_free_balance_be(&zoran, 200_000_000u32.into());
		let faucet_name = FaucetNameType::from_str("Some Faucet Name").unwrap();
		let amount: BalanceOf<T> = 100_000_000u32.into();
		let drip_amount: BalanceOf<T> = 10_000u32.into();
		let whitelist = BoundedVec::try_from(vec![cid; 10]).unwrap();
		assert!(<Faucets<T>>::iter().next().is_none());
		FaucetPallet::<T>::create_faucet(RawOrigin::Signed(zoran).into(), faucet_name, amount, whitelist, drip_amount).ok();
		let faucet_account = <Faucets<T>>::iter().next().unwrap().0;
		FaucetPallet::<T>::drip(RawOrigin::Signed(dripper.clone()).into(), faucet_account.clone(), cid, cindex).ok();

	}: _(RawOrigin::Root, faucet_account, beneficiary.clone())
	verify {
		assert_eq!(<T as pallet::Config>::Currency::free_balance(&beneficiary), 99_990_000u32.into());
	}


	close_faucet {
		let cid = create_community::<T>();
		let cindex = 10;
		let zoran = account("zoran", 1, 1);
		let dripper = account::<T::AccountId>("dripper", 2, 2);
		let beneficiary = account::<T::AccountId>("beneficiary", 3, 3);

		frame_support::storage::unhashed::put_raw(&participant_reputation((cid, cindex), &dripper), &Reputation::VerifiedUnlinked.encode());
		<T as pallet::Config>::Currency::make_free_balance_be(&zoran, 100_000_000u32.into());
		let faucet_name = FaucetNameType::from_str("Some Faucet Name").unwrap();
		let amount: BalanceOf<T> = 25_000_000u32.into();
		let drip_amount: BalanceOf<T> = 10_000_000u32.into();
		let whitelist = BoundedVec::try_from(vec![cid; 10]).unwrap();
		assert!(<Faucets<T>>::iter().next().is_none());
		FaucetPallet::<T>::create_faucet(RawOrigin::Signed(zoran.clone()).into(), faucet_name, amount, whitelist, drip_amount).ok();
		let faucet_account = <Faucets<T>>::iter().next().unwrap().0;
		FaucetPallet::<T>::drip(RawOrigin::Signed(dripper.clone()).into(), faucet_account.clone(), cid, cindex).ok();
		assert_eq!(<T as pallet::Config>::Currency::free_balance(&faucet_account), 15_000_000u32.into());
	}: _(RawOrigin::Signed(zoran), faucet_account.clone())
	verify {
		assert_eq!(<T as pallet::Config>::Currency::free_balance(&faucet_account), 0u32.into());
	}

	set_reserve_amount {
	}: _(RawOrigin::Root, 1337u32.into())
	verify {
		assert_eq!(<ReserveAmount<T>>::get(), 1337u32.into());
	}
}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
