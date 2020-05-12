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


use crate::{Module, Trait, CeremonyPhaseType, GenesisConfig};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, OnFinalize, OnInitialize},
	Perbill,
};
use primitives::H256;
use support::{assert_ok, impl_outer_origin, impl_outer_event, parameter_types,
        storage::StorageValue};
use runtime_io::TestExternalities;
use inherents::ProvideInherent;
use std::ops::Rem;

impl_outer_origin! {
	pub enum Origin for TestRuntime {}
}

mod simple_event {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum TestEvent for TestRuntime {
		simple_event,
		system<T>,
	}
}

parameter_types! {
	pub const MomentsPerDay: u64 = 86_400_000; // [ms/d]
}
impl Trait for TestRuntime {
    type Event = TestEvent;
    type OnCeremonyPhaseChange = ();
    type MomentsPerDay = MomentsPerDay;
}

type AccountId = u64;

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TestRuntime;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Trait for TestRuntime {
	type Origin = Origin;
	type Index = u64;
	type Call = ();
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
	type ModuleToIndex = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();    
}

parameter_types! {
    pub const MinimumPeriod: u64 = 1;
}
impl timestamp::Trait for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = EncointerScheduler;
	type MinimumPeriod = MinimumPeriod;
}

pub struct ExtBuilder;

const MASTER: AccountId = 0;

impl ExtBuilder {
    pub fn build() -> TestExternalities {
        let mut storage = system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        GenesisConfig::<TestRuntime> {
            current_phase: CeremonyPhaseType::REGISTERING,
            current_ceremony_index: 1,
            ceremony_master: MASTER,
            phase_durations: vec![
                (CeremonyPhaseType::REGISTERING, 86_400_000),
                (CeremonyPhaseType::ASSIGNING, 86_400_000),
                (CeremonyPhaseType::ATTESTING, 86_400_000),
            ]
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}
pub type System = system::Module<TestRuntime>;
pub type Timestamp = timestamp::Module<TestRuntime>;
pub type EncointerScheduler = Module<TestRuntime>;

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


#[test]
fn ceremony_phase_statemachine_works() {
    ExtBuilder::build().execute_with(|| {
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(
            MASTER
        )));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(
            MASTER
        )));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ATTESTING
        );
        assert_ok!(EncointerScheduler::next_phase(Origin::signed(
            MASTER
        )));
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
    });
}

#[test]
fn timestamp_callback_works() {
    ExtBuilder::build().execute_with(|| {
        //large offset since 1970 to when first block is generated
        const GENESIS_TIME: u64 = 1_585_058_843_000;
        const ONE_DAY: u64 = 86_400_000;
        System::set_block_number(0);
        
        let _ = Timestamp::dispatch(<timestamp::Module<TestRuntime> as ProvideInherent>::Call::
            set(GENESIS_TIME), Origin::NONE);

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::next_phase_timestamp(), 
            (GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)) + ONE_DAY);

        run_to_block(1);

        let _ = Timestamp::dispatch(<timestamp::Module<TestRuntime> as ProvideInherent>::Call::
            set(GENESIS_TIME + ONE_DAY), Origin::NONE);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );

        run_to_block(2);

        let _ = Timestamp::dispatch(<timestamp::Module<TestRuntime> as ProvideInherent>::Call::
            set(GENESIS_TIME + 2 * ONE_DAY), Origin::NONE);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ATTESTING
        );

        run_to_block(3);
        let _ = Timestamp::dispatch(<timestamp::Module<TestRuntime> as ProvideInherent>::Call::
            set(GENESIS_TIME + 3 * ONE_DAY), Origin::NONE);
        assert_eq!(EncointerScheduler::current_ceremony_index(), 2);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );

    });
}