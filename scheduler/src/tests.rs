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

use crate::{CeremonyPhaseType, GenesisConfig, Module, Trait};
use frame_support::traits::UnfilteredDispatchable;
use frame_support::{
    assert_ok, impl_outer_event,
    traits::{OnFinalize, OnInitialize},
};
use inherents::ProvideInherent;
use runtime_io::TestExternalities;
use sp_core::H256;
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
};
use std::ops::Rem;

use test_utils::*;

type AccountId = u64;

mod simple_event {
    pub use crate::Event;
}

impl_outer_event! {
    pub enum TestEvent for TestRuntime {
        simple_event,
        frame_system<T>,
    }
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

pub type System = frame_system::Module<TestRuntime>;
pub type Timestamp = timestamp::Module<TestRuntime>;
pub type EncointerScheduler = Module<TestRuntime>;

impl_frame_system!(TestRuntime, TestEvent);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_outer_origin_for_runtime!(TestRuntime);

impl Trait for TestRuntime {
    type Event = TestEvent;
    type OnCeremonyPhaseChange = ();
    type MomentsPerDay = MomentsPerDay;
}

pub struct ExtBuilder {
    equal_phase_duration: Moment,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {
            equal_phase_duration: 86_400_000,
        }
    }
}

const MASTER: AccountId = 0;

impl ExtBuilder {
    pub fn equal_phase_duration(mut self, d: Moment) -> Self {
        self.equal_phase_duration = d;
        self
    }

    pub fn build(&self) -> TestExternalities {
        let mut storage = frame_system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        GenesisConfig::<TestRuntime> {
            current_phase: CeremonyPhaseType::REGISTERING,
            current_ceremony_index: 1,
            ceremony_master: MASTER,
            phase_durations: vec![
                (CeremonyPhaseType::REGISTERING, self.equal_phase_duration),
                (CeremonyPhaseType::ASSIGNING, self.equal_phase_duration),
                (CeremonyPhaseType::ATTESTING, self.equal_phase_duration),
            ],
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

/// Run until a particular block.
pub fn run_to_block(n: u64) {
    while System::block_number() < n {
        if System::block_number() > 1 {
            System::on_finalize(System::block_number());
        }
        Timestamp::on_finalize(System::block_number());
        System::set_block_number(System::block_number() + 1);
        System::on_initialize(System::block_number());
    }
}

pub fn set_timestamp(t: u64) {
    let _ = <timestamp::Module<TestRuntime> as ProvideInherent>::Call::set(t)
        .dispatch_bypass_filter(Origin::none());
}

#[test]
fn ceremony_phase_statemachine_works() {
    ExtBuilder::default().build().execute_with(|| {
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ATTESTING
        );
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
    });
}

#[test]
fn timestamp_callback_works() {
    ExtBuilder::default().build().execute_with(|| {
        //large offset since 1970 to when first block is generated
        const GENESIS_TIME: u64 = 1_585_058_843_000;
        const ONE_DAY: u64 = 86_400_000;
        System::set_block_number(0);

        set_timestamp(GENESIS_TIME);

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(
            EncointerScheduler::next_phase_timestamp(),
            (GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)) + ONE_DAY
        );

        run_to_block(1);
        set_timestamp(GENESIS_TIME + ONE_DAY);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );

        run_to_block(2);
        set_timestamp(GENESIS_TIME + 2 * ONE_DAY);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ATTESTING
        );

        run_to_block(3);
        set_timestamp(GENESIS_TIME + 3 * ONE_DAY);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
    });
}

const TEN_MIN: u64 = 600_000;
const ONE_DAY: u64 = 86_400_000;

#[test]
fn push_one_day_works() {
    ExtBuilder::default()
        .equal_phase_duration(ONE_DAY)
        .build()
        .execute_with(|| {
            let genesis_time: u64 = 0 * TEN_MIN + 1;

            System::set_block_number(0);
            set_timestamp(genesis_time);

            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * ONE_DAY
            );

            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );

            run_to_block(1);
            set_timestamp(genesis_time + TEN_MIN);

            assert_ok!(EncointerScheduler::push_by_one_day(Origin::signed(MASTER)));
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 2 * ONE_DAY
            );

            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
        });
}
#[test]
fn resync_catches_up_short_cycle_times_at_genesis_during_first_registering_phase() {
    ExtBuilder::default()
        .equal_phase_duration(TEN_MIN)
        .build()
        .execute_with(|| {
            // CASE1: genesis happens during first REGISTERING phase of the day
            let genesis_time: u64 = 0 * TEN_MIN + 1;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * TEN_MIN
            );
        });
}

#[test]
fn resync_catches_up_short_cycle_times_at_genesis_during_third_registering_phase() {
    ExtBuilder::default()
        .equal_phase_duration(TEN_MIN)
        .build()
        .execute_with(|| {
            // CASE2: genesis happens during 3rd REGISTERING phase of the day
            let genesis_time: u64 = 6 * TEN_MIN + 1;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 7 * TEN_MIN
            );
        });
}

#[test]
fn resync_catches_up_short_cycle_times_at_genesis_during_third_assigning_phase() {
    ExtBuilder::default()
        .equal_phase_duration(TEN_MIN)
        .build()
        .execute_with(|| {
            // CASE3: genesis happens during 3rd ASSIGNING phase of the day
            let genesis_time: u64 = 7 * TEN_MIN + 1;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 10 * TEN_MIN
            );
        });
}

#[test]
fn resync_catches_up_short_cycle_times_at_genesis_during_third_attesting_phase() {
    ExtBuilder::default()
        .equal_phase_duration(TEN_MIN)
        .build()
        .execute_with(|| {
            // CASE4: genesis happens during 3rd ATTESTING phase of the day
            let genesis_time: u64 = 8 * TEN_MIN + 1;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 10 * TEN_MIN
            );
        });
}

#[test]
fn resync_after_next_phase_works() {
    ExtBuilder::default()
        .equal_phase_duration(ONE_DAY)
        .build()
        .execute_with(|| {
            let genesis_time: u64 = 0;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * ONE_DAY
            );

            run_to_block(1);
            set_timestamp(genesis_time + TEN_MIN);

            // now use next_phase manually
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ASSIGNING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 2 * ONE_DAY
            );
            // this means that we merely anticipated the ASSIGNING_PHASE. NExt ATTESTING will still start as if next_phase() had not been called

            run_to_block(2);
            set_timestamp(genesis_time + 2 * TEN_MIN);

            // again
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ATTESTING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 3 * ONE_DAY
            );
            // this means that we merely anticipated the ATTESTING phase. NExt REGISTERING will still start as if next_phase() had not been called

            run_to_block(3);
            set_timestamp(genesis_time + 3 * TEN_MIN);

            // again
            // because we would skip an entire Cycle now, we resync to the next
            // even next_phase_timestamp in the future
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * ONE_DAY
            );
            // now the next ASSIGNING phase starts exactly at the time it would have startet if next_phase had not been called.
            // But the ceremony index increased by one
        });
}

#[test]
fn resync_after_next_phase_works_during_assigning() {
    ExtBuilder::default()
        .equal_phase_duration(ONE_DAY)
        .build()
        .execute_with(|| {
            let genesis_time: u64 = 0;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * ONE_DAY
            );

            run_to_block(1);
            set_timestamp(genesis_time + ONE_DAY + TEN_MIN);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ASSIGNING
            );

            // now use next_phase manually
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));

            assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ASSIGNING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 2 * ONE_DAY
            );
        });
}
#[test]
fn resync_after_next_phase_works_during_attesting() {
    ExtBuilder::default()
        .equal_phase_duration(ONE_DAY)
        .build()
        .execute_with(|| {
            let genesis_time: u64 = 0;

            System::set_block_number(0);

            set_timestamp(genesis_time);

            assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::REGISTERING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 1 * ONE_DAY
            );

            run_to_block(1);
            set_timestamp(genesis_time + 1 * ONE_DAY + TEN_MIN);

            run_to_block(2);
            set_timestamp(genesis_time + 2 * ONE_DAY + TEN_MIN);

            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ATTESTING
            );

            // now use next_phase manually
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));
            assert_ok!(EncointerScheduler::next_phase(Origin::signed(MASTER)));

            assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
            assert_eq!(
                EncointerScheduler::current_phase(),
                CeremonyPhaseType::ATTESTING
            );
            assert_eq!(
                EncointerScheduler::next_phase_timestamp(),
                (genesis_time - genesis_time.rem(ONE_DAY)) + 3 * ONE_DAY
            );
        });
}
