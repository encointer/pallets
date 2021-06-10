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
use crate::{Config, Module};
use encointer_primitives::communities::CommunityIdentifier;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};

use test_utils::{helpers::register_test_community, *};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl_frame_system!(TestRuntime);
impl_encointer_balances!(TestRuntime);
impl_encointer_communities!(TestRuntime);
impl_outer_origin_for_runtime!(TestRuntime);

impl Config for TestRuntime {
    type Event = ();
}

pub type EncointerBazaar = Module<TestRuntime>;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

fn create_cid() -> CommunityIdentifier {
    return register_test_community::<TestRuntime>(None, 2);
}

fn alice() -> AccountId { AccountKeyring::Alice.into() }

fn bob() -> AccountId { AccountKeyring::Bob.into() }

fn url() -> String {
    return "https://encointer.org".to_string();
}

fn url1() -> String {
    return "https://substrate.dev".to_string();
}

fn url2() -> String {
    return "https://polkadot.network".to_string();
}

#[test]
fn create_business() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();

        assert!(EncointerBazaar::create_business(Origin::signed(alice()), cid, url()).is_ok());
        assert!(EncointerBazaar::create_business(Origin::signed(bob()), cid, url1()).is_ok());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData { url: url(), last_oid: 1 });
        assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData { url: url1(), last_oid: 1 });
    });
}

#[test]
fn create_business_with_invalid_cid() {
    ExtBuilder::build().execute_with(|| {
        assert!(EncointerBazaar::create_business(Origin::signed(alice()), CommunityIdentifier::zero(), url()).is_err());

        assert_eq!(EncointerBazaar::business_registry(CommunityIdentifier::zero(), alice()), BusinessData::default());
    });
}

#[test]
fn create_business_duplicate() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();

        assert!(EncointerBazaar::create_business(Origin::signed(alice()), cid, url()).is_ok());
        assert!(EncointerBazaar::create_business(Origin::signed(alice()), cid, url1()).is_err());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData { url: url(), last_oid: 1 });
    });
}

#[test]
fn update_business() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });

        assert!(EncointerBazaar::update_business(Origin::signed(alice()), cid, url1()).is_ok());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData { url: url1(), last_oid: 2 });
    });
}

#[test]
fn update_business_inexistent() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 3 });

        assert!(EncointerBazaar::update_business(Origin::signed(bob()), cid, url1()).is_err());
        assert!(EncointerBazaar::update_business(Origin::signed(alice()), CommunityIdentifier::zero(), url1()).is_err());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData { url: url(), last_oid: 3 });
    });
}

#[test]
fn delete_business() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData { url: url1(), last_oid: 3 });

        assert!(EncointerBazaar::delete_business(Origin::signed(alice()), cid).is_ok());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
        assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData { url: url1(), last_oid: 3 });
    });
}

#[test]
fn delete_business_inexistent() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData { url: url1(), last_oid: 2 });

        assert!(EncointerBazaar::delete_business(Origin::signed(alice()), cid).is_err());
        assert!(EncointerBazaar::delete_business(Origin::signed(bob()), CommunityIdentifier::zero()).is_err());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
        assert_eq!(EncointerBazaar::business_registry(cid, bob()), BusinessData { url: url1(), last_oid: 2 });
    });
}

#[test]
fn create_offering() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 1 });

        assert!(EncointerBazaar::create_offering(Origin::signed(alice()), cid, url1()).is_ok());
        assert!(EncointerBazaar::create_offering(Origin::signed(alice()), cid, url2()).is_ok());

        //TODO get offering identifier from thrown event 
        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1),
                   OfferingData { url: url1() });
        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 2),
                   OfferingData { url: url2() });
    });
}

#[test]
fn create_offering_inexistent_business() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 1 });

        assert!(EncointerBazaar::create_offering(Origin::signed(bob()), cid, url1()).is_err());

        //TODO assert events
    });
}

#[test]
fn create_offering_inexistent_community() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 1 });

        assert!(EncointerBazaar::create_offering(Origin::signed(alice()), CommunityIdentifier::zero(), url1()).is_err());

        //TODO assert events
    });
}

#[test]
fn update_offering() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1,
                                                OfferingData { url: url() });

        assert!(EncointerBazaar::update_offering(Origin::signed(alice()), cid, 1, url1()).is_ok());

        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1),
                   OfferingData { url: url1() });
    });
}

#[test]
fn update_offering_inexistent() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1,
                                                OfferingData { url: url() });

        assert!(EncointerBazaar::update_offering(Origin::signed(bob()), cid, 1, url1()).is_err());
        assert!(EncointerBazaar::update_offering(Origin::signed(alice()), cid, 0, url1()).is_err());
        assert!(EncointerBazaar::update_offering(Origin::signed(alice()), CommunityIdentifier::zero(), 1, url1()).is_err());
    });
}

#[test]
fn delete_offering() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1,
                                                OfferingData { url: url() });

        assert!(EncointerBazaar::delete_offering(Origin::signed(alice()), cid, 1).is_ok());

        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1),
                   OfferingData::default());
    });
}

#[test]
fn delete_offering_inexistent() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1,
                                                OfferingData { url: url() });

        assert!(EncointerBazaar::delete_offering(Origin::signed(bob()), cid, 1).is_err());
        assert!(EncointerBazaar::delete_offering(Origin::signed(alice()), cid, 0).is_err());
        assert!(EncointerBazaar::delete_offering(Origin::signed(alice()), CommunityIdentifier::zero(), 1).is_err());
    });
}

#[test]
fn delete_business_with_offerings() {
    ExtBuilder::build().execute_with(|| {
        let cid = create_cid();
        BusinessRegistry::<TestRuntime>::insert(cid, alice(), BusinessData { url: url(), last_oid: 2 });
        BusinessRegistry::<TestRuntime>::insert(cid, bob(), BusinessData { url: url1(), last_oid: 2 });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1, OfferingData { url: url() });
        OfferingRegistry::<TestRuntime>::insert(BusinessIdentifier { community_identifier: cid, business_account: bob() }, 1, OfferingData { url: url1() });

        assert!(EncointerBazaar::delete_business(Origin::signed(alice()), cid).is_ok());

        assert_eq!(EncointerBazaar::business_registry(cid, alice()), BusinessData::default());
        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: alice() }, 1), OfferingData::default());
        assert_eq!(EncointerBazaar::offering_registry(BusinessIdentifier { community_identifier: cid, business_account: bob() }, 1), OfferingData { url: url1() });
    });
}