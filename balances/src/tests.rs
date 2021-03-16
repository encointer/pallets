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

#![cfg(test)]

use super::*;
use fixed::{traits::LossyInto, transcendental::exp};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use mock::{EncointerBalances, ExtBuilder, System, TestEvent, TestRuntime};

use encointer_primitives::{
    balances::consts::DEFAULT_DEMURRAGE,
    communities::{CommunityIdentifier, Demurrage},
};
use test_utils::{helpers::register_test_community, AccountKeyring};

#[test]
fn issue_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        let cid = CommunityIdentifier::default();
        let alice = AccountKeyring::Alice.to_account_id();
        assert_ok!(EncointerBalances::issue(
            cid,
            &alice,
            BalanceType::from_num(50.1)
        ));
        assert_eq!(
            EncointerBalances::balance(cid, &alice),
            BalanceType::from_num(50.1)
        );
        assert_eq!(
            EncointerBalances::total_issuance(cid),
            BalanceType::from_num(50.1)
        );
    });
}

#[test]
fn burn_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        let cid = CommunityIdentifier::default();
        let alice = AccountKeyring::Alice.to_account_id();
        assert_ok!(EncointerBalances::issue(
            cid,
            &alice,
            BalanceType::from_num(50)
        ));
        assert_ok!(EncointerBalances::burn(
            cid,
            &alice,
            BalanceType::from_num(20)
        ));
        assert_eq!(
            EncointerBalances::balance(cid, &alice),
            BalanceType::from_num(30)
        );
        assert_eq!(
            EncointerBalances::total_issuance(cid),
            BalanceType::from_num(30)
        );
        assert_noop!(
            EncointerBalances::burn(cid, &alice, BalanceType::from_num(31)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}

#[test]
fn transfer_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());

        let alice = AccountKeyring::Alice.to_account_id();
        let bob = AccountKeyring::Bob.to_account_id();
        let cid = CommunityIdentifier::default();
        assert_ok!(EncointerBalances::issue(
            cid,
            &alice,
            BalanceType::from_num(50)
        ));
        assert_ok!(EncointerBalances::transfer(
            Some(alice.clone()).into(),
            bob.clone(),
            cid,
            BalanceType::from_num(9.999)
        ));

        let balance: f64 = EncointerBalances::balance(cid, &alice).lossy_into();
        assert_relative_eq!(balance, 40.001, epsilon = 1.0e-9);

        let balance: f64 = EncointerBalances::balance(cid, &bob).lossy_into();
        assert_relative_eq!(balance, 9.999, epsilon = 1.0e-9);

        let balance: f64 = EncointerBalances::total_issuance(cid).lossy_into();
        assert_relative_eq!(balance, 50.0, epsilon = 1.0e-9);

        let transferred_event = TestEvent::tokens(RawEvent::Transferred(
            cid,
            alice.clone(),
            bob.clone(),
            BalanceType::from_num(9.999),
        ));
        assert!(System::events()
            .iter()
            .any(|record| record.event == transferred_event));

        assert_noop!(
            EncointerBalances::transfer(Some(alice).into(), bob, cid, BalanceType::from_num(60)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}

#[test]
fn demurrage_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        let alice = AccountKeyring::Alice.to_account_id();
        let cid = register_test_community::<TestRuntime>(None, 3);
        System::set_block_number(0);
        assert_ok!(EncointerBalances::issue(
            cid,
            &alice,
            BalanceType::from_num(1)
        ));
        System::set_block_number(1);
        assert_eq!(
            EncointerBalances::balance(cid, &alice),
            exp::<BalanceType, BalanceType>(-Demurrage::from_bits(DEFAULT_DEMURRAGE)).unwrap()
        );
        //one year later
        System::set_block_number(86400 / 5 * 356);
        let result: f64 = EncointerBalances::balance(cid, &alice).lossy_into();
        assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
        let result: f64 = EncointerBalances::total_issuance(cid).lossy_into();
        assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
    });
}

#[test]
fn transfer_with_demurrage_exceeding_amount_should_fail() {
    let alice = AccountKeyring::Alice.to_account_id();
    let bob = AccountKeyring::Bob.to_account_id();
    ExtBuilder::default().build().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 3);
        System::set_block_number(0);
        assert_ok!(EncointerBalances::issue(
            cid,
            &alice,
            BalanceType::from_num(100)
        ));
        //one year later
        System::set_block_number(86400 / 5 * 356);
        // balance should now be 50
        assert_noop!(
            EncointerBalances::transfer(Some(alice).into(), bob, cid, BalanceType::from_num(60)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}
