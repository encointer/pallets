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
use encointer_communities::CurrencyIdentifier;
use fixed::{traits::LossyInto, transcendental::exp};
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use mock::{
    register_test_currency, EncointerBalances, EncointerCurrencies, ExtBuilder, System, TestEvent,
    TestRuntime, ALICE, BOB,
};

#[test]
fn issue_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        let cid = CurrencyIdentifier::default();
        assert_ok!(EncointerBalances::issue(
            cid,
            &ALICE,
            BalanceType::from_num(50.1)
        ));
        assert_eq!(
            EncointerBalances::balance(cid, &ALICE),
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
        let cid = CurrencyIdentifier::default();
        assert_ok!(EncointerBalances::issue(
            cid,
            &ALICE,
            BalanceType::from_num(50)
        ));
        assert_ok!(EncointerBalances::burn(
            cid,
            &ALICE,
            BalanceType::from_num(20)
        ));
        assert_eq!(
            EncointerBalances::balance(cid, &ALICE),
            BalanceType::from_num(30)
        );
        assert_eq!(
            EncointerBalances::total_issuance(cid),
            BalanceType::from_num(30)
        );
        assert_noop!(
            EncointerBalances::burn(cid, &ALICE, BalanceType::from_num(31)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}

#[test]
fn transfer_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());

        let cid = CurrencyIdentifier::default();
        assert_ok!(EncointerBalances::issue(
            cid,
            &ALICE,
            BalanceType::from_num(50)
        ));
        assert_ok!(EncointerBalances::transfer(
            Some(ALICE).into(),
            BOB,
            cid,
            BalanceType::from_num(9.999)
        ));

        let balance: f64 = EncointerBalances::balance(cid, &ALICE).lossy_into();
        assert_relative_eq!(balance, 40.001, epsilon = 1.0e-9);

        let balance: f64 = EncointerBalances::balance(cid, &BOB).lossy_into();
        assert_relative_eq!(balance, 9.999, epsilon = 1.0e-9);

        let balance: f64 = EncointerBalances::total_issuance(cid).lossy_into();
        assert_relative_eq!(balance, 50.0, epsilon = 1.0e-9);

        let transferred_event = TestEvent::tokens(RawEvent::Transferred(
            cid,
            ALICE,
            BOB,
            BalanceType::from_num(9.999),
        ));
        assert!(System::events()
            .iter()
            .any(|record| record.event == transferred_event));

        assert_noop!(
            EncointerBalances::transfer(Some(ALICE).into(), BOB, cid, BalanceType::from_num(60)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}

#[test]
fn demurrage_should_work() {
    ExtBuilder::default().build().execute_with(|| {
        let cid = register_test_currency();
        System::set_block_number(0);
        assert_ok!(EncointerBalances::issue(
            cid,
            &ALICE,
            BalanceType::from_num(1)
        ));
        System::set_block_number(1);
        assert_eq!(
            EncointerBalances::balance(cid, &ALICE),
            exp::<BalanceType, BalanceType>(
                -EncointerCurrencies::currency_properties(cid).demurrage_per_block
            )
            .unwrap()
        );
        //one year later
        System::set_block_number(86400 / 5 * 356);
        let result: f64 = EncointerBalances::balance(cid, &ALICE).lossy_into();
        assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
        let result: f64 = EncointerBalances::total_issuance(cid).lossy_into();
        assert_abs_diff_eq!(result, 0.5, epsilon = 1.0e-12);
    });
}

#[test]
fn transfer_with_demurrage_exceeding_amount_should_fail() {
    ExtBuilder::default().build().execute_with(|| {
        let cid = register_test_currency();
        System::set_block_number(0);
        assert_ok!(EncointerBalances::issue(
            cid,
            &ALICE,
            BalanceType::from_num(100)
        ));
        //one year later
        System::set_block_number(86400 / 5 * 356);
        // balance should now be 50
        assert_noop!(
            EncointerBalances::transfer(Some(ALICE).into(), BOB, cid, BalanceType::from_num(60)),
            Error::<TestRuntime>::BalanceTooLow,
        );
    });
}
