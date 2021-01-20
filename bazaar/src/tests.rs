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
//use crate::{GenesisConfig, Module, Trait};
use crate::{Module, Trait};
use codec::Encode;
use encointer_communities::{CommunityIdentifier, Degree, Location};
use frame_support::traits::Get;
use frame_support::{assert_ok, impl_outer_origin, parameter_types};
use sp_core::{hashing::blake2_256, sr25519, H256};
use sp_keyring::AccountKeyring;
use sp_runtime::traits::Verify;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    Perbill,
};
use std::cell::RefCell;

/// The signature type used by accounts/transactions.
pub type Signature = sr25519::Signature;
/// An identifier for an account on this system.
pub type AccountId = <Signature as Verify>::Signer;

thread_local! {
    static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(2);
}
pub type BlockNumber = u64;
pub type Balance = u64;

pub struct ExistentialDeposit;
impl Get<u64> for ExistentialDeposit {
    fn get() -> u64 {
        EXISTENTIAL_DEPOSIT.with(|v| *v.borrow())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

impl Trait for TestRuntime {
    type Event = ();
}

impl encointer_balances::Trait for TestRuntime {
    type Event = ();
}

pub type EncointerBazaar = Module<TestRuntime>;

impl encointer_communities::Trait for TestRuntime {
    type Event = ();
}
pub type EncointerCurrencies = encointer_communities::Module<TestRuntime>;

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: u32 = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for TestRuntime {
    type BaseCallFilter = ();
    type Origin = Origin;
    type Index = u64;
    type Call = ();
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type DbWeight = ();
    type BlockExecutionWeight = ();
    type ExtrinsicBaseWeight = ();
    type MaximumBlockLength = MaximumBlockLength;
    type MaximumExtrinsicWeight = MaximumBlockWeight;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type AccountData = balances::AccountData<u64>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type PalletInfo = ();
}

pub type System = frame_system::Module<TestRuntime>;

parameter_types! {
    pub const TransferFee: Balance = 0;
    pub const CreationFee: Balance = 0;
    pub const TransactionBaseFee: u64 = 0;
    pub const TransactionByteFee: u64 = 0;
}

impl balances::Trait for TestRuntime {
    type Balance = Balance;
    type Event = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type WeightInfo = ();
    type MaxLocks = ();
}

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

/// register a simple test currency with 3 meetup locations and well known bootstrappers
fn register_test_currency() -> CommunityIdentifier {
    // all well-known keys are boottrappers for easy testen afterwards
    let alice = AccountId::from(AccountKeyring::Alice);
    let bob = AccountId::from(AccountKeyring::Bob);
    let charlie = AccountId::from(AccountKeyring::Charlie);

    let a = Location::default(); // 0, 0

    let b = Location {
        lat: Degree::from_num(1),
        lon: Degree::from_num(1),
    };
    let c = Location {
        lat: Degree::from_num(2),
        lon: Degree::from_num(2),
    };
    let loc = vec![a, b, c];
    let bs = vec![alice.clone(), bob.clone(), charlie.clone()];
    assert_ok!(EncointerCurrencies::new_currency(
        Origin::signed(alice.clone()),
        loc.clone(),
        bs.clone()
    ));
    CommunityIdentifier::from(blake2_256(&(loc, bs).encode()))
}

#[test]
fn create_new_shop_works() {
    ExtBuilder::build().execute_with(|| {
        // initialisation
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");
        // upload dummy store to blockchain
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_ok()
        );

        // get shops from blockchain
        let shops = EncointerBazaar::shop_registry(cid);
        let alices_shops = EncointerBazaar::shops_owned(cid, alice);
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
        let cid = CommunityIdentifier::from(blake2_256(&(0, alice).encode())); // fails to register cid

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
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        // upload dummy store to blockchain
        assert!(
            EncointerBazaar::new_shop(Origin::signed(alice.clone()), cid, alice_shop.clone())
                .is_ok()
        );

        // get shops from blockchain
        let mut shops = EncointerBazaar::shop_registry(cid);
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice);
        // assert that shop was added
        assert!(shops.contains(&alice_shop));
        assert!(alices_shops.contains(&alice_shop));
        // assert that the shop is owned by alice
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop), alice);

        // remove shop from blockchain
        assert!(EncointerBazaar::remove_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop.clone()
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
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop_one = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTA");
        let alice_shop_two = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        // upload stores to blockchain
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_one.clone()
        )
        .is_ok());
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone()
        )
        .is_ok());

        // get shops from blockchain
        let mut shops = EncointerBazaar::shop_registry(cid);
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice);
        // assert that shops were added
        assert!(shops.contains(&alice_shop_one));
        assert!(shops.contains(&alice_shop_two));

        assert!(alices_shops.contains(&alice_shop_one));
        assert!(alices_shops.contains(&alice_shop_two));

        // assert that the shops are owned by alice
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop_one), alice);
        assert_eq!(EncointerBazaar::shop_owner(&cid, &alice_shop_two), alice);

        // delete shop two
        assert!(EncointerBazaar::remove_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone()
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
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        let alice_shop_one = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");
        let alice_shop_two = ShopIdentifier::from("QmW6WLLhUPsosBcKebejveknjrSQjZjq5eYFVBRfugygTB");

        //let cid = CommunityIdentifier::from(blake2_256(&(loc.clone(), bs.clone()).encode()))

        // upload stores to blockchain
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_one.clone()
        )
        .is_ok());
        assert!(EncointerBazaar::new_shop(
            Origin::signed(alice.clone()),
            cid,
            alice_shop_two.clone()
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
        let cid = register_test_currency();
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
        let mut alices_shops = EncointerBazaar::shops_owned(cid, alice);
        let bobs_shops = EncointerBazaar::shops_owned(cid, bob);
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
        let cid = register_test_currency();
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
