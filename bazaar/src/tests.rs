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

//! Unit tests for the tokens module.

use super::*;
use frame_support::dispatch::DispatchResultWithPostInfo;
use mock::{new_test_ext, EncointerBazaar, Origin, System, TestRuntime};

use encointer_primitives::communities::CommunityIdentifier;
use test_utils::{
	helpers::{assert_last_event, register_test_community},
	*,
};

fn create_cid() -> CommunityIdentifier {
	return register_test_community::<TestRuntime>(None, 0.0, 0.0)
}

fn alice() -> AccountId {
	AccountKeyring::Alice.into()
}

fn bob() -> AccountId {
	AccountKeyring::Bob.into()
}

fn url() -> String {
	return "https://encointer.org".to_string()
}

fn url1() -> String {
	return "https://substrate.dev".to_string()
}

fn url2() -> String {
	return "https://polkadot.network".to_string()
}

fn assert_error(actual: DispatchResultWithPostInfo, expected: Error<TestRuntime>) {
	assert_eq!(
		match actual.clone().unwrap_err().error {
			sp_runtime::DispatchError::Module(module_error) => module_error.message.unwrap(),
			_ => panic!(),
		},
		expected.as_str()
	);
}

#[test]
fn create_new_business_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();

		assert!(EncointerBazaar::create_business(Origin::signed(alice()), cid, url()).is_ok());
		assert_last_event::<TestRuntime>(Event::BusinessCreated(cid.clone(), alice()).into());

		assert!(EncointerBazaar::create_business(Origin::signed(bob()), cid, url1()).is_ok());
		assert_last_event::<TestRuntime>(Event::BusinessCreated(cid.clone(), bob()).into());

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::new(url(), 1));
		assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData::new(url1(), 1));

		assert_eq!(System::events().len(), 3);
	});
}

#[test]
fn create_business_with_invalid_cid_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		assert_error(
			EncointerBazaar::create_business(
				Origin::signed(alice()),
				CommunityIdentifier::default(),
				url(),
			),
			Error::<TestRuntime>::NonexistentCommunity,
		);

		assert_eq!(
			EncointerBazaar::business_registry(CommunityIdentifier::default(), alice()),
			BusinessData::default()
		);

		assert_eq!(System::events().len(), 0);
	});
}

#[test]
fn create_business_duplicate_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();

		assert!(EncointerBazaar::create_business(Origin::signed(alice()), cid, url()).is_ok());
		assert_error(
			EncointerBazaar::create_business(Origin::signed(alice()), cid, url1()),
			Error::<TestRuntime>::ExistingBusiness,
		);
		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::new(url(), 1));

		assert_eq!(System::events().len(), 2);
		assert_last_event::<TestRuntime>(Event::BusinessCreated(cid.clone(), alice()).into());
	});
}

#[test]
fn update_existing_business_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));

		assert!(EncointerBazaar::update_business(Origin::signed(alice()), cid, url1()).is_ok());

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::new(url1(), 2));

		assert_eq!(System::events().len(), 2);
		assert_last_event::<TestRuntime>(Event::BusinessUpdated(cid.clone(), alice()).into());
	});
}

#[test]
fn update_inexistent_business_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 3));

		assert_error(
			EncointerBazaar::update_business(Origin::signed(bob()), cid, url1()),
			Error::<TestRuntime>::NonexistentBusiness,
		);
		assert_error(
			EncointerBazaar::update_business(
				Origin::signed(alice()),
				CommunityIdentifier::default(),
				url1(),
			),
			Error::<TestRuntime>::NonexistentBusiness,
		);

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::new(url(), 3));

		assert_eq!(System::events().len(), 1);
	});
}

#[test]
fn delete_existing_business_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData::new(url1(), 3));

		assert!(EncointerBazaar::delete_business(Origin::signed(alice()), cid).is_ok());

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
		assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData::new(url1(), 3));

		assert_eq!(System::events().len(), 2);
		assert_last_event::<TestRuntime>(Event::BusinessDeleted(cid.clone(), alice()).into());
	});
}

#[test]
fn delete_inexistent_business_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData::new(url1(), 2));

		assert_error(
			EncointerBazaar::delete_business(Origin::signed(alice()), cid),
			Error::<TestRuntime>::NonexistentBusiness,
		);
		assert_error(
			EncointerBazaar::delete_business(Origin::signed(bob()), CommunityIdentifier::default()),
			Error::<TestRuntime>::NonexistentBusiness,
		);

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
		assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData::new(url1(), 2));

		assert_eq!(System::events().len(), 1);
	});
}

fn get_oid(test_event: &mock::Event) -> u32 {
	let raw_event = match test_event {
		mock::Event::EncointerBazaar(event) => event,
		_ => panic!(),
	};
	let oid = match raw_event {
		Event::OfferingCreated(_, _, oid) => oid,
		_ => panic!(),
	};
	return *oid
}

#[test]
fn create_new_offering_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 1));

		assert!(EncointerBazaar::create_offering(Origin::signed(alice()), cid, url1()).is_ok());
		assert!(EncointerBazaar::create_offering(Origin::signed(alice()), cid, url2()).is_ok());

		let records = System::events();
		assert_eq!(records.len(), 3);
		assert_eq!(
			EncointerBazaar::offering_registry(
				BusinessIdentifier::new(cid, alice()),
				get_oid(&records.get(1).unwrap().event)
			)
			.url,
			url1()
		);
		assert_eq!(
			EncointerBazaar::offering_registry(
				BusinessIdentifier::new(cid, alice()),
				get_oid(&records.get(2).unwrap().event)
			)
			.url,
			url2()
		);
	});
}

#[test]
fn create_offering_for_inexistent_business_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 1));

		assert_error(
			EncointerBazaar::create_offering(Origin::signed(bob()), cid, url1()),
			Error::<TestRuntime>::NonexistentBusiness,
		);

		assert_eq!(System::events().len(), 1);
	});
}

#[test]
fn update_existing_offering_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, alice()),
			1,
			OfferingData::new(url()),
		);

		assert!(EncointerBazaar::update_offering(Origin::signed(alice()), cid, 1, url1()).is_ok());

		let records = System::events();
		assert_eq!(records.len(), 2);
		assert_last_event::<TestRuntime>(Event::OfferingUpdated(cid.clone(), alice(), 1).into());

		assert_eq!(
			EncointerBazaar::offering_registry(BusinessIdentifier::new(cid, alice()), 1).url,
			url1()
		);
	});
}

#[test]
fn update_inexistent_offering_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, alice()),
			1,
			OfferingData::new(url()),
		);

		assert_error(
			EncointerBazaar::update_offering(Origin::signed(bob()), cid, 1, url1()),
			Error::<TestRuntime>::NonexistentOffering,
		);
		assert_error(
			EncointerBazaar::update_offering(Origin::signed(alice()), cid, 0, url1()),
			Error::<TestRuntime>::NonexistentOffering,
		);
		assert_error(
			EncointerBazaar::update_offering(
				Origin::signed(alice()),
				CommunityIdentifier::default(),
				1,
				url1(),
			),
			Error::<TestRuntime>::NonexistentOffering,
		);

		assert_eq!(System::events().len(), 1);
	});
}

#[test]
fn delete_existing_offering_is_ok() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, alice()),
			1,
			OfferingData::new(url()),
		);

		assert!(EncointerBazaar::delete_offering(Origin::signed(alice()), cid, 1).is_ok());

		assert_eq!(
			EncointerBazaar::offering_registry(BusinessIdentifier::new(cid, alice()), 1),
			OfferingData::default()
		);

		assert_eq!(System::events().len(), 2);
		assert_last_event::<TestRuntime>(Event::OfferingDeleted(cid.clone(), alice(), 1).into());
	});
}

#[test]
fn delete_inexistent_offering_is_err() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, alice()),
			1,
			OfferingData::new(url()),
		);

		assert_error(
			EncointerBazaar::delete_offering(Origin::signed(bob()), cid, 1),
			Error::<TestRuntime>::NonexistentOffering,
		);
		assert_error(
			EncointerBazaar::delete_offering(Origin::signed(alice()), cid, 0),
			Error::<TestRuntime>::NonexistentOffering,
		);
		assert_error(
			EncointerBazaar::delete_offering(
				Origin::signed(alice()),
				CommunityIdentifier::default(),
				1,
			),
			Error::<TestRuntime>::NonexistentOffering,
		);

		assert_eq!(System::events().len(), 1);
	});
}

#[test]
fn when_deleting_business_delete_all_its_offerings() {
	new_test_ext().execute_with(|| {
		System::set_block_number(System::block_number() + 1);
		let cid = create_cid();
		BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData::new(url(), 2));
		BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData::new(url1(), 2));
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, alice()),
			1,
			OfferingData::new(url()),
		);
		OfferingRegistry::<TestRuntime>::insert(
			BusinessIdentifier::new(cid, bob()),
			1,
			OfferingData::new(url1()),
		);

		assert!(EncointerBazaar::delete_business(Origin::signed(alice()), cid).is_ok());

		assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
		assert_eq!(
			EncointerBazaar::offering_registry(BusinessIdentifier::new(cid, alice()), 1),
			OfferingData::default()
		);
		assert_eq!(
			EncointerBazaar::offering_registry(BusinessIdentifier::new(cid, bob()), 1),
			OfferingData::new(url1())
		);

		assert_eq!(System::events().len(), 2);
		assert_last_event::<TestRuntime>(Event::BusinessDeleted(cid.clone(), alice()).into());
	});
}
