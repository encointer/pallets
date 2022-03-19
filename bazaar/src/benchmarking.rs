use crate::*;
use encointer_primitives::communities::{
	CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Degree, Location,
};
use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_std::borrow::ToOwned;

fn test_url() -> PalletString {
	"https://test.com".to_owned().into()
}

fn example_url() -> PalletString {
	"https://example.com".to_owned().into()
}

fn create_community<T: Config>() -> CommunityIdentifier {
	let alice: T::AccountId = account("alice", 1, 1);
	let bob: T::AccountId = account("bob", 2, 2);
	let charlie: T::AccountId = account("charlie", 3, 3);

	let location = Location { lat: Degree::from_num(1i32), lon: Degree::from_num(1i32) };

	let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
	let community_meta: CommunityMetadataType = CommunityMetadataType {
		name: "Default".into(),
		symbol: "DEF".into(),
		..Default::default()
	};
	encointer_communities::Pallet::<T>::new_community(
		RawOrigin::Root.into(),
		location,
		bs.clone(),
		community_meta.clone(),
		None,
		None,
	)
	.ok();

	let cid = CommunityIdentifier::new(location, bs).unwrap();
	cid
}

benchmarks! {
	create_business {
	let alice: T::AccountId = account("alice", 1, 1);
	let cid = create_community::<T>();
	}: _(RawOrigin::Signed(alice.clone()), cid, test_url())
	verify {
		assert_eq!(Pallet::<T>::business_registry(cid, &alice), BusinessData::new(test_url(), 1));
	}

	update_business {
		let alice: T::AccountId = account("alice", 1, 1);
		let cid = create_community::<T>();
		BusinessRegistry::<T>::insert(cid, &alice, BusinessData::new(test_url(), 2));
	}: _(RawOrigin::Signed(alice.clone()), cid , example_url())
	verify{
		assert_eq!(Pallet::<T>::business_registry(cid, &alice), BusinessData::new(example_url(), 2));
	}

	delete_business {
		let alice: T::AccountId = account("alice", 1, 1);
		let cid = create_community::<T>();
		BusinessRegistry::<T>::insert(cid, &alice, BusinessData::new(test_url(), 2));
	} : _(RawOrigin::Signed(alice.clone()), cid)
	verify {
		assert_eq!(Pallet::<T>::business_registry(cid, &alice), BusinessData::default());
	}

	create_offering {
		let alice: T::AccountId = account("alice", 1, 1);
		let cid = create_community::<T>();
		let business_identifier = BusinessIdentifier::new(cid, alice.clone());
		BusinessRegistry::<T>::insert(cid, &alice, BusinessData::new(test_url(), 1));
		assert!(OfferingRegistry::<T>::iter_prefix_values(business_identifier.clone()).count() == 0);
	} : _(RawOrigin::Signed(alice.clone()), cid, example_url())
	verify {
		assert!(OfferingRegistry::<T>::iter_prefix_values(business_identifier.clone()).count() == 1);
	}

	update_offering {
		let alice: T::AccountId = account("alice", 1, 1);
		let cid = create_community::<T>();
		let business_identifier = BusinessIdentifier::new(cid, alice.clone());
		BusinessRegistry::<T>::insert(cid, &alice, BusinessData::new(test_url(), 1));
		OfferingRegistry::<T>::insert(business_identifier.clone(), 1, OfferingData::new(test_url()));
		assert_eq!(OfferingRegistry::<T>::get(business_identifier.clone(), 1), OfferingData::new(test_url()));
	} : _(RawOrigin::Signed(alice.clone()), cid, 1, example_url())
	verify {
		assert_eq!(OfferingRegistry::<T>::get(business_identifier.clone(), 1), OfferingData::new(example_url()));
	}

	delete_offering {
		let alice: T::AccountId = account("alice", 1, 1);
		let cid = create_community::<T>();
		let business_identifier = BusinessIdentifier::new(cid, alice.clone());
		BusinessRegistry::<T>::insert(cid, &alice, BusinessData::new(test_url(), 1));
		OfferingRegistry::<T>::insert(business_identifier.clone(), 1, OfferingData::new(test_url()));
		assert!(OfferingRegistry::<T>::iter_prefix_values(business_identifier.clone()).count() == 1);

	} : _(RawOrigin::Signed(alice.clone()), cid, 1)
	verify {
		assert!(OfferingRegistry::<T>::iter_prefix_values(business_identifier.clone()).count() == 0);
	}

}

impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::TestRuntime);
