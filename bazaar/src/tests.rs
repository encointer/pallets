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
use codec::Encode;
use encointer_primitives::communities::CommunityIdentifier;
use sp_core::{hashing::blake2_256, H256};
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
fn create_new_shop_works() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");
        // upload dummy store to blockchain
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_ok()
        );

        // get shops from blockchain
        let shops = EncointerBazaar::shop_registry(cid);
        let alices_shops = EncointerBazaar::shops_owned(cid, alice.clone());
        // assert that shop was added
        assert!(shops.contains(&alice_shop));
        assert!(alices_shops.contains(&alice_shop));
        // assert that the shop is owned by alice
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop), alice);
    });
}

#[test]
fn create_new_shop_with_bad_cid_fails() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");
        let cid = CommunityIdentifier::from(blake2_256(&(0, alice.clone()).encode())); // fails to register cid

        // assert that upload fails
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_err()
        );

        // get shops from blockchain
        let shops = EncointerBazaar::shop_registry(cid);
        let alices_shops = EncointerBazaar::shops_owned(cid, alice);

        // assert that shop was not added
        assert_eq!(shops.contains(&alice_shop), false);
        assert_eq!(alices_shops.contains(&alice_shop), false);
    });
}

#[test]
fn removal_of_shop_works() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        // upload dummy store to blockchain
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_ok()
        );

        // get shops from blockchain
        let mut shops = EncointerBazaar::shop_registry(cid);
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice.clone());
        // assert that shop was added
        assert!(shops.contains(&alice_shop));
        assert!(alices_shops.contains(&alice_shop));
        // assert that the shop is owned by alice
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop), alice);

        // remove shop from blockchain
        assert!(EncointerBazaar::remove_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop.clone(),
        )
            .is_ok());

        // update local shop list
        shops = EncointerBazaar::shop_registry(cid);
        alices_shops = EncointerBazaar::shops_owned(cid, alice);

        // assert that shop was removed
        assert_eq!(shops.contains(&alice_shop), false);
        assert_eq!(alices_shops.contains(&alice_shop), false);
        // TODO: How to assert that hash key is exisiting or empty?
        //assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop), None);
    });
}

#[test]
fn alices_store_are_differentiated() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop_one = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTA");
        let alice_shop_two = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        // upload stores to blockchain
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_one.clone(),
        )
            .is_ok());
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone(),
        )
            .is_ok());

        // get shops from blockchain
        let mut shops = EncointerBazaar::shop_registry(cid);
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice.clone());
        // assert that shops were added
        assert!(shops.contains(&alice_shop_one));
        assert!(shops.contains(&alice_shop_two));

        assert!(alices_shops.contains(&alice_shop_one));
        assert!(alices_shops.contains(&alice_shop_two));

        // assert that the shops are owned by alice
        assert_eq!(
            EncointerBazaar::shop_owner(&cid, &alice_shop_one),
            alice.clone()
        );
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop_two), alice);

        // delete shop two
        assert!(EncointerBazaar::remove_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone(),
        )
            .is_ok());

        // assert that shop two was removed and shop one still exisits
        shops = EncointerBazaar::shop_registry(cid);
        alices_shops = EncointerBazaar::shops_owned(cid, alice);

        assert!(shops.contains(&alice_shop_one));
        assert_eq!(shops.contains(&alice_shop_two), false);

        assert!(alices_shops.contains(&alice_shop_one));
        assert_eq!(alices_shops.contains(&alice_shop_two), false);
    });
}

#[test]
fn stores_cannot_be_created_twice() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop_one = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");
        let alice_shop_two = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        //let cid = CommunityIdentifier::from(blake2_256(&(loc.clone(), bs.clone()).encode()))

        // upload stores to blockchain
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_one.clone(),
        )
            .is_ok());
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone(),
        )
            .is_err());

        // get shops from blockchain
        let shops = EncointerBazaar::shop_registry(cid);
        let alices_shops = EncointerBazaar::shops_owned(cid, alice);

        // Assert that the shop has been uploaded correctly
        assert!(shops.contains(&alice_shop_one));
        assert!(alices_shops.contains(&alice_shop_one));
    });
}

#[test]
fn bob_cannot_remove_alices_store() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTA");
        let bob_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfAgygTB");

        // upload stores to blockchain
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_ok()
        );
        assert!(
            EncointerBazaar::new_shop(Origin::signed(bob.clone()), cid, bob_shop.clone()).is_ok()
        );

        // get shops from blockchain
        let mut shops = EncointerBazaar::shop_registry(cid);
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice.clone());
        let bobs_shops = EncointerBazaar::shops_owned(cid, bob.clone());
        // assert that shops were added
        assert!(shops.contains(&alice_shop));
        assert!(shops.contains(&bob_shop));

        assert!(alices_shops.contains(&alice_shop));
        assert!(bobs_shops.contains(&bob_shop));

        // assert that the shops are owned by alice or bob respective
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop), alice);
        assert_eq!(EncointerBazaar::shop_owner(&cid, &bob_shop), bob);

        // assert that bob can not delete alices shop
        assert!(
            EncointerBazaar::remove_shop(Origin::signed(bob.clone()), cid, alice_shop.clone())
                .is_err()
        );

        // assert that shop has not been deleted
        alices_shops = EncointerBazaar::shops_owned(cid, alice);
        shops = EncointerBazaar::shop_registry(cid);

        assert!(alices_shops.contains(&alice_shop));
        assert!(shops.contains(&alice_shop));
    });
}

#[test]
fn create_oversized_shop_fails() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_community::<TestRuntime>(None, 2);
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTBB");

        // assert that upload fails
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_err()
        );

        // get shops from blockchain
        let shops = EncointerBazaar::shop_registry(cid);
        let alices_shops = EncointerBazaar::shops_owned(cid, alice);

        // assert that shop was not added
        assert_eq!(shops.contains(&alice_shop), false);
        assert_eq!(alices_shops.contains(&alice_shop), false);
    });
}
