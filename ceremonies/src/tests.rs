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


//extern crate externalities;
//extern crate test_client;
//extern crate node_primitives;

use super::*;
use crate::{GenesisConfig, Module, Trait};
use encointer_currencies::{CurrencyIdentifier, Location, Degree};
use encointer_scheduler::{CeremonyPhaseType, CeremonyIndexType, OnCeremonyPhaseChange};
use externalities::set_and_run_with_externalities;
use primitives::crypto::Ss58Codec;
use primitives::{hashing::blake2_256, sr25519, Blake2Hasher, Pair, Public, H256};
use sp_runtime::traits::{CheckedAdd, IdentifyAccount, Member, Verify, OnFinalize, OnInitialize};
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup},
    MultiSignature, Perbill,
};
use inherents::ProvideInherent;
use std::{cell::RefCell, collections::HashSet, ops::Rem};
use support::traits::{Currency, FindAuthor, Get, LockIdentifier};
use support::{assert_ok, impl_outer_event, impl_outer_origin, parameter_types};
use sp_keyring::AccountKeyring;

const NONE: u64 = 0;
const GENESIS_TIME: u64 = 1_585_058_843_000;
const ONE_DAY: u64 = 86_400_000;
const BLOCKTIME: u64 = 3_600_000; //1h per block
const TIME_TOLERANCE: u64 = 1000; // [ms]
const LOCATION_TOLERANCE: u32 = 100; // [m]
const ZERO: BalanceType = BalanceType::from_bits(0x0);

thread_local! {
    static EXISTENTIAL_DEPOSIT: RefCell<u64> = RefCell::new(0);
}
/// The signature type used by accounts/transactions.
pub type Signature = sr25519::Signature;
/// An identifier for an account on this system.
pub type AccountId = <Signature as Verify>::Signer;

pub type BlockNumber = u64;
pub type Balance = u64;

type TestAttestation = Attestation<Signature, AccountId, Moment>;
type TestProofOfAttendance = ProofOfAttendance<Signature, AccountId>;

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
    type Public = AccountId;
    type Signature = Signature;
}

pub type EncointerCeremonies = Module<TestRuntime>;

impl encointer_currencies::Trait for TestRuntime {
    type Event = ();
}

pub type EncointerCurrencies = encointer_currencies::Module<TestRuntime>;

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
    type BlockNumber = BlockNumber;
    type Hash = H256;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
	type ModuleToIndex = ();
	type AccountData = balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();          
}

pub type System = system::Module<TestRuntime>;

impl encointer_balances::Trait for TestRuntime {
	type Event = ();
}

pub type EncointerBalances = encointer_balances::Module<TestRuntime>;

parameter_types! {
	pub const MomentsPerDay: u64 = 86_400_000; // [ms/d]
}
impl encointer_scheduler::Trait for TestRuntime {
    type Event = ();
    type OnCeremonyPhaseChange = Module<TestRuntime>; //OnCeremonyPhaseChange;
    type MomentsPerDay = MomentsPerDay;
}
pub type EncointerScheduler = encointer_scheduler::Module<TestRuntime>;

type Moment = u64;
parameter_types! {
    pub const MinimumPeriod: Moment = 1;
}
impl timestamp::Trait for TestRuntime {
	type Moment = Moment;
	type OnTimestampSet = EncointerScheduler;
	type MinimumPeriod = MinimumPeriod;
}
pub type Timestamp = timestamp::Module<TestRuntime>;

//type AccountPublic = <Signature as Verify>::Signer;

pub struct ExtBuilder;

impl ExtBuilder {
    pub fn build() -> runtime_io::TestExternalities {
        let mut storage = system::GenesisConfig::default()
            .build_storage::<TestRuntime>()
            .unwrap();
        encointer_currencies::GenesisConfig::<TestRuntime> {
            currency_master: AccountId::from(AccountKeyring::Alice),
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        encointer_scheduler::GenesisConfig::<TestRuntime> {
            current_phase: CeremonyPhaseType::REGISTERING,
            current_ceremony_index: 1,
            ceremony_master: AccountId::from(AccountKeyring::Alice),
            phase_durations: vec![
                (CeremonyPhaseType::REGISTERING, ONE_DAY),
                (CeremonyPhaseType::ASSIGNING, ONE_DAY),
                (CeremonyPhaseType::ATTESTING, ONE_DAY),
            ]
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        GenesisConfig::<TestRuntime> {
            ceremony_reward: BalanceType::from_num(1),
            location_tolerance: LOCATION_TOLERANCE, // [m]
            time_tolerance: TIME_TOLERANCE, // [ms]
        }
        .assimilate_storage(&mut storage)
        .unwrap();
        runtime_io::TestExternalities::from(storage)
    }
}

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

/// Run until a particular block.
fn run_to_block(n: u64) {
	while System::block_number() < n {
		if System::block_number() > 1 {
            System::on_finalize(System::block_number());
        }
        let _ = Timestamp::dispatch(<timestamp::Module<TestRuntime> as ProvideInherent>::Call::
            set(GENESIS_TIME + BLOCKTIME * n), Origin::NONE);
        Timestamp::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		System::on_initialize(System::block_number());
	}
}

/// Progress blocks until the phase changes
fn run_to_next_phase() {
    let phase = EncointerScheduler::current_phase();
    let mut blocknr = System::block_number();
    while phase == EncointerScheduler::current_phase() {
        blocknr += 1;
        run_to_block(blocknr);
    }
}

/// get correct meetup time for a certain cid and meetup
fn correct_meetup_time(cid: &CurrencyIdentifier, mindex: MeetupIndexType) -> Moment {
    //assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::ATTESTING);
    let cindex = EncointerScheduler::current_ceremony_index() as u64;
    let mlon: f64 = EncointerCeremonies::get_meetup_location(cid, mindex).unwrap().lon.lossy_into();
    let t = GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) 
        + cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::REGISTERING)
        + cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::ASSIGNING)
        + (cindex-1) * EncointerScheduler::phase_durations(CeremonyPhaseType::ATTESTING)    
        + ONE_DAY - (mlon / 360.0 * ONE_DAY as f64) as u64 ;
    t.into()
}

/// generate a fresh claim for claimant and sign it by attester
fn meetup_claim_sign(
    claimant: AccountId,
    attester: sr25519::Pair,
    cid: CurrencyIdentifier,
    cindex: CeremonyIndexType,
    mindex: MeetupIndexType,
    location: Location,
    timestamp: Moment,
    n_participants: u32,
) -> TestAttestation {
    let claim = ClaimOfAttendance {
        claimant_public: claimant.clone(),
        currency_identifier: cid,
        ceremony_index: cindex,
        meetup_index: mindex,
        location,
        timestamp,
        number_of_participants_confirmed: n_participants,
    };
    TestAttestation {
        claim: claim.clone(),
        signature: Signature::from(attester.sign(&claim.encode())),
        public: get_accountid(&attester),
    }
}

/// generate a proof of attendance based on previous reputation
fn prove_attendance(
    prover: AccountId,
    cid: CurrencyIdentifier,
    cindex: CeremonyIndexType,
    attendee: &sr25519::Pair,
) -> TestProofOfAttendance {
    let msg = (prover.clone(), cindex);
    ProofOfAttendance {
        prover_public: prover,
        currency_identifier: cid,
        ceremony_index: cindex,
        attendee_public: get_accountid(&attendee),
        attendee_signature: Signature::from(attendee.sign(&msg.encode())),
    }
}

/// shortcut to register well-known keys for current ceremony
fn register_alice_bob_ferdie(cid: CurrencyIdentifier) {
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Alice.pair())),
        cid,
        None
    ));
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Bob.pair())),
        cid,
        None
    ));
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Ferdie.pair())),
        cid,
        None
    ));
}

/// shortcut to register well-known keys for current ceremony
fn register_charlie_dave_eve(cid: CurrencyIdentifier) {
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Charlie.pair())),
        cid,
        None
    ));
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Dave.pair())),
        cid,
        None
    ));
    assert_ok!(EncointerCeremonies::register_participant(
        Origin::signed(get_accountid(&AccountKeyring::Eve.pair())),
        cid,
        None
    ));
}

/// shorthand for attesting one claimant by many attesters. register all attestation to chain
fn gets_attested_by(
    claimant: AccountId,
    attestors: Vec<sr25519::Pair>,
    cid: CurrencyIdentifier,
    cindex: CeremonyIndexType,
    mindex: MeetupIndexType,
    location: Location,
    timestamp: Moment,
    n_participants: u32,
) {
    let mut attestations: Vec<TestAttestation> = vec![];
    for a in attestors {
        attestations.insert(
            0,
            meetup_claim_sign(
                claimant.clone(),
                a.clone(),
                cid,
                cindex,
                mindex,
                location,
                timestamp,
                n_participants,
            ),
        );
    }
    assert_ok!(EncointerCeremonies::register_attestations(
        Origin::signed(claimant),
        attestations.clone()
    ));
}

/// shorthand to convert Pair to AccountId
fn get_accountid(pair: &sr25519::Pair) -> AccountId {
    AccountId::from(pair.public()).into_account()
}

/// register a simple test currency with 3 meetup locations and well known bootstrappers
fn register_test_currency() -> CurrencyIdentifier {
    // all well-known keys are boottrappers for easy testen afterwards
    let alice = AccountId::from(AccountKeyring::Alice);
    let bob = AccountId::from(AccountKeyring::Bob);
    let charlie = AccountId::from(AccountKeyring::Charlie);
    let dave = AccountId::from(AccountKeyring::Dave);
    let eve = AccountId::from(AccountKeyring::Eve);
    let ferdie = AccountId::from(AccountKeyring::Ferdie);
    
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
    let bs = vec![
        alice.clone(),
        bob.clone(),
        charlie.clone(),
        dave.clone(),
        eve.clone(),
        ferdie.clone(),
    ];
    assert_ok!(EncointerCurrencies::new_currency(
        Origin::signed(alice.clone()),
        loc.clone(),
        bs.clone()
    ));
    CurrencyIdentifier::from(blake2_256(&(loc, bs).encode()))
}

/// perform bootstrapping ceremony for test currency with well known bootstrappers
fn perform_bootstrapping_ceremony() -> CurrencyIdentifier {
    let cid = register_test_currency();
    register_alice_bob_ferdie(cid);
    register_charlie_dave_eve(cid);
    let master = AccountId::from(AccountKeyring::Alice);
    let alice = AccountKeyring::Alice.pair();
    let bob = AccountKeyring::Bob.pair();
    let charlie = AccountKeyring::Charlie.pair();
    let dave = AccountKeyring::Dave.pair();
    let eve = AccountKeyring::Eve.pair();
    let ferdie = AccountKeyring::Ferdie.pair();

    let cindex = EncointerScheduler::current_ceremony_index();

    run_to_next_phase();
    // ASSIGNING
    run_to_next_phase();
    // ATTESTING
    let loc = Location::default();
    let time = correct_meetup_time(&cid, 1);
    gets_attested_by(
        get_accountid(&alice),
        vec![
            bob.clone(),
            charlie.clone(),
            dave.clone(),
            eve.clone(),
            ferdie.clone(),
        ],
        cid,
        1,
        1,
        loc,
        time,
        6,
    );
    gets_attested_by(
        get_accountid(&bob),
        vec![
            alice.clone(),
            charlie.clone(),
            dave.clone(),
            eve.clone(),
            ferdie.clone(),
        ],
        cid,
        1,
        1,
        loc, 
        time,
        6,
    );
    gets_attested_by(
        get_accountid(&charlie),
        vec![
            alice.clone(),
            bob.clone(),
            dave.clone(),
            eve.clone(),
            ferdie.clone(),
        ],
        cid,
        1,
        1,
        loc, 
        time,
        6,
    );
    gets_attested_by(
        get_accountid(&dave),
        vec![
            alice.clone(),
            bob.clone(),
            charlie.clone(),
            eve.clone(),
            ferdie.clone(),
        ],
        cid,
        1,
        1,
        loc, 
        time,
        6,
    );
    gets_attested_by(
        get_accountid(&eve),
        vec![
            alice.clone(),
            bob.clone(),
            charlie.clone(),
            dave.clone(),
            ferdie.clone(),
        ],
        cid,
        1,
        1,
        loc, 
        time,
        6,
    );
    gets_attested_by(
        get_accountid(&ferdie),
        vec![
            alice.clone(),
            bob.clone(),
            charlie.clone(),
            dave.clone(),
            eve.clone(),
        ],
        cid,
        1,
        1,
        loc, 
        time,
        6,
    );

    run_to_next_phase();
    // REGISTERING
    cid
}

/// perform full attestation of all participants for a given meetup
fn fully_attest_meetup(cid: CurrencyIdentifier, keys: Vec<sr25519::Pair>, mindex: MeetupIndexType) {
    let cindex = EncointerScheduler::current_ceremony_index();
    let meetup = EncointerCeremonies::meetup_registry((cid, cindex), mindex);
    for p in meetup.iter() {
        let mut others = Vec::with_capacity(meetup.len() - 1);
        println!("participant {}", p.to_ss58check());
        for o in meetup.iter() {
            println!("attestor {}", o.to_ss58check());
            if o == p {
                println!("same same");
                continue;
            }
            for pair in keys.iter() {
                println!("checking {}", pair.public().to_ss58check());
                if get_accountid(pair) == *o {
                    others.push(pair.clone());
                }
            }
        }
        println!("  length of attestors: {}", others.len());
        let loc = EncointerCurrencies::locations(&cid)[(mindex - 1) as usize];
        let time = correct_meetup_time(&cid, mindex);
        gets_attested_by(
            (*p).clone(),
            others,
            cid,
            cindex,
            mindex,
            loc, 
            time,
            meetup.len() as u32,
        );
    }
}

// unit tests ////////////////////////////////////////

#[test]
fn registering_participant_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let cindex = EncointerScheduler::current_ceremony_index();
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 0);
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        ));
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 1);
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(bob.clone()),
            cid,
            None
        ));
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 2);
        assert_eq!(
            EncointerCeremonies::participant_index((cid, cindex), &bob),
            2
        );
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &1),
            alice
        );
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &2),
            bob
        );
    });
}

#[test]
fn registering_participant_twice_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let alice = AccountId::from(AccountKeyring::Alice);
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        ));
        assert!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        )
        .is_err());
    });
}

#[test]
fn registering_participant_in_wrong_phase_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountId::from(AccountKeyring::Alice);
        run_to_next_phase();
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );
        assert!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        )
        .is_err());
    });
}

#[test]
fn assigning_meetup_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let ferdie = AccountId::from(AccountKeyring::Ferdie);
        let cindex = EncointerScheduler::current_ceremony_index();
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        ));
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(bob.clone()),
            cid,
            None
        ));
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(ferdie.clone()),
            cid,
            None
        ));
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 3);
        //omitting phase change here!
        EncointerCeremonies::assign_meetups();
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);
        let meetup = EncointerCeremonies::meetup_registry((cid, cindex), 1);
        assert_eq!(meetup.len(), 3);
        assert!(meetup.contains(&alice));
        assert!(meetup.contains(&bob));
        assert!(meetup.contains(&ferdie));

        assert_eq!(EncointerCeremonies::meetup_index((cid, cindex), &alice), 1);
        assert_eq!(EncointerCeremonies::meetup_index((cid, cindex), &bob), 1);
        assert_eq!(EncointerCeremonies::meetup_index((cid, cindex), &ferdie), 1);
    });
}

#[test]
fn verify_attestation_signatue_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let claimant = AccountKeyring::Alice.pair();
        let attester = AccountKeyring::Bob.pair();

        let claim = ClaimOfAttendance {
            claimant_public: get_accountid(&claimant),
            currency_identifier: cid,
            ceremony_index: 1,
            meetup_index: 1,
            location: Location::default(),
            timestamp: correct_meetup_time(&cid, 1),
            number_of_participants_confirmed: 3,
        };
        let attestation_good = TestAttestation {
            claim: claim.clone(),
            signature: Signature::from(attester.sign(&claim.encode())),
            public: get_accountid(&attester),
        };
        let attestation_wrong_signature = TestAttestation {
            claim: claim.clone(),
            signature: Signature::from(claimant.sign(&claim.encode())),
            public: get_accountid(&attester),
        };
        let attestation_wrong_signer = TestAttestation {
            claim: claim.clone(),
            signature: Signature::from(attester.sign(&claim.encode())),
            public: get_accountid(&claimant),
        };
        assert_ok!(EncointerCeremonies::verify_attestation_signature(
            attestation_good
        ));
        assert!(
            EncointerCeremonies::verify_attestation_signature(attestation_wrong_signature).is_err()
        );
        assert!(
            EncointerCeremonies::verify_attestation_signature(attestation_wrong_signer).is_err()
        );
    });
}

#[test]
fn register_attestations_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        assert_eq!(
            EncointerCeremonies::meetup_index((cid, cindex), &get_accountid(&alice)),
            1
        );
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        gets_attested_by(
            get_accountid(&alice),
            vec![bob.clone(), ferdie.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            3,
        );
        gets_attested_by(
            get_accountid(&bob),
            vec![alice.clone(), ferdie.clone()],
            cid,
            1,
            1,
            loc,
            time,
            3,
        );

        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
        assert_eq!(
            EncointerCeremonies::attestation_index((cid, cindex), &get_accountid(&bob)),
            2
        );
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &2);
        assert!(wit_vec.len() == 2);
        assert!(wit_vec.contains(&get_accountid(&alice)));
        assert!(wit_vec.contains(&get_accountid(&ferdie)));

        // TEST: re-registering must overwrite previous entry
        gets_attested_by(
            get_accountid(&alice),
            vec![bob.clone(), ferdie.clone()],
            cid,
            1,
            1,
            loc,
            time,
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
    });
}

#[test]
fn register_attestations_for_non_participant_fails_silently() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        
        gets_attested_by(
            get_accountid(&alice),
            vec![bob.clone(), alice.clone()],
            cid,
            1,
            1,
            Location::default(),
            correct_meetup_time(&cid, 1),
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&get_accountid(&alice)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn register_attestations_for_non_participant_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let mut eve_attestations: Vec<TestAttestation> = vec![];
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        eve_attestations.insert(
            0,
            meetup_claim_sign(get_accountid(&eve), alice.clone(), cid, 1, 1, loc, time, 3),
        );
        eve_attestations.insert(
            1,
            meetup_claim_sign(get_accountid(&eve), ferdie.clone(), cid, 1, 1, loc, time, 3),
        );
        assert!(EncointerCeremonies::register_attestations(
            Origin::signed(get_accountid(&eve)),
            eve_attestations.clone()
        )
        .is_err());
    });
}

#[test]
fn register_attestations_with_non_participant_fails_silently() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        gets_attested_by(
            get_accountid(&alice),
            vec![bob.clone(), eve.clone()],
            cid,
            1,
            1,
            Location::default(),
            correct_meetup_time(&cid, 1),
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&get_accountid(&eve)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn register_attestations_with_wrong_meetup_index_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        let mut alice_attestations: Vec<TestAttestation> = vec![];
        alice_attestations.insert(
            0,
            meetup_claim_sign(get_accountid(&alice), bob.clone(), cid, 1, 1, loc, time, 3),
        );
        let claim = ClaimOfAttendance {
            claimant_public: get_accountid(&alice),
            currency_identifier: cid,
            ceremony_index: 1,
            // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
            location: Location::default(),
            timestamp: time,
            meetup_index: 1 + 99,
            number_of_participants_confirmed: 3,
        };
        alice_attestations.insert(
            1,
            TestAttestation {
                claim: claim.clone(),
                signature: Signature::from(ferdie.sign(&claim.encode())),
                public: get_accountid(&ferdie),
            },
        );
        assert_ok!(EncointerCeremonies::register_attestations(
            Origin::signed(get_accountid(&alice)),
            alice_attestations
        ));
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&get_accountid(&ferdie)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn register_attestations_with_wrong_ceremony_index_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        let mut alice_attestations: Vec<TestAttestation> = vec![];
        alice_attestations.insert(
            0,
            meetup_claim_sign(get_accountid(&alice), bob.clone(), cid, 1, 1, loc, time, 3),
        );
        let claim = ClaimOfAttendance {
            claimant_public: get_accountid(&alice),
            currency_identifier: cid,
            // !!!!!!!!!!!!!!!!!!!!!!!!!!
            ceremony_index: 99,
            meetup_index: 1,
            location: Location::default(),
            timestamp: time,
            number_of_participants_confirmed: 3,
        };
        alice_attestations.insert(
            1,
            TestAttestation {
                claim: claim.clone(),
                signature: Signature::from(ferdie.sign(&claim.encode())),
                public: get_accountid(&ferdie),
            },
        );
        assert_ok!(EncointerCeremonies::register_attestations(
            Origin::signed(get_accountid(&alice)),
            alice_attestations
        ));
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&get_accountid(&ferdie)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn register_attestations_with_wrong_timestamp_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        // too late!
        let time = correct_meetup_time(&cid, 1) + TIME_TOLERANCE + 1;
        let mut alice_attestations: Vec<TestAttestation> = vec![];
        alice_attestations.push(
            meetup_claim_sign(get_accountid(&alice), bob.clone(), cid, 1, 1, loc, time, 3),
        );
        alice_attestations.push(
            meetup_claim_sign(get_accountid(&alice), ferdie.clone(), cid, 1, 1, loc, time, 3),
        );
        assert!(EncointerCeremonies::register_attestations(
            Origin::signed(get_accountid(&alice)),
            alice_attestations
        ).is_err());
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.len() == 0);
    });
}

#[test]
fn register_attestations_with_wrong_location_fails() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING

        // too far away!
        let mut loc = Location::default();
        loc.lon += Degree::from_num(0.01); // ~1.11km east of meetup location along equator
        let time = correct_meetup_time(&cid, 1);
        let mut alice_attestations: Vec<TestAttestation> = vec![];
        alice_attestations.push(
            meetup_claim_sign(get_accountid(&alice), bob.clone(), cid, 1, 1, loc, time, 3),
        );
        alice_attestations.push(
            meetup_claim_sign(get_accountid(&alice), ferdie.clone(), cid, 1, 1, loc, time, 3),
        );
        assert!(EncointerCeremonies::register_attestations(
            Origin::signed(get_accountid(&alice)),
            alice_attestations
        ).is_err());
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.len() == 0);
    });
}

#[test]
fn ballot_meetup_n_votes_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        register_charlie_dave_eve(cid);

        run_to_next_phase();
        // ASSIGNING
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        gets_attested_by(get_accountid(&alice), vec![bob.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&bob), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&charlie), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&dave), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&eve), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&ferdie), vec![dave.clone()], cid, 1, 1, loc, time, 6);
        assert!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 1) == Some((5, 5)));

        gets_attested_by(get_accountid(&alice), vec![bob.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&bob), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&charlie), vec![alice.clone()], cid, 1, 1, loc, time, 4);
        gets_attested_by(get_accountid(&dave), vec![alice.clone()], cid, 1, 1, loc, time, 4);
        gets_attested_by(get_accountid(&eve), vec![alice.clone()], cid, 1, 1, loc, time, 6);
        gets_attested_by(get_accountid(&ferdie), vec![dave.clone()], cid, 1, 1, loc, time, 6);
        assert!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 1) == None);

        gets_attested_by(get_accountid(&alice), vec![bob.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&bob), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&charlie), vec![alice.clone()], cid, 1, 1, loc, time, 5);
        gets_attested_by(get_accountid(&dave), vec![alice.clone()], cid, 1, 1, loc, time, 4);
        gets_attested_by(get_accountid(&eve), vec![alice.clone()], cid, 1, 1, loc, time, 6);
        gets_attested_by(get_accountid(&ferdie), vec![dave.clone()], cid, 1, 1, loc, time, 6);
        assert!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 1) == Some((5, 3)));
    });
}

#[test]
fn issue_reward_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        register_charlie_dave_eve(cid);

        run_to_next_phase();
        // ASSIGNING
        run_to_next_phase();
        // ATTESTING
        // ferdi doesn't show up
        // eve signs no one else
        // charlie collects incomplete signatures
        // dave signs ferdi and reports wrong number of participants
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        gets_attested_by(
            get_accountid(&alice),
            vec![bob.clone(), charlie.clone(), dave.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            5,
        );
        gets_attested_by(
            get_accountid(&bob),
            vec![alice.clone(), charlie.clone(), dave.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            5,
        );
        gets_attested_by(
            get_accountid(&charlie),
            vec![alice.clone(), bob.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            5,
        );
        gets_attested_by(
            get_accountid(&dave),
            vec![alice.clone(), bob.clone(), charlie.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            6,
        );
        gets_attested_by(
            get_accountid(&eve),
            vec![alice.clone(), bob.clone(), charlie.clone(), dave.clone()],
            cid,
            1,
            1,
            loc, 
            time,
            5,
        );
        gets_attested_by(
            get_accountid(&ferdie), 
            vec![dave.clone()], cid, 1, 1, loc, time, 6);

        assert_eq!(EncointerBalances::balance(cid, &get_accountid(&alice)), ZERO);

        run_to_next_phase();
        // REGISTERING

        let result: f64 = EncointerBalances::balance(cid, &get_accountid(&alice)).lossy_into();
        assert_abs_diff_eq!(
            result,
            EncointerCeremonies::ceremony_reward().lossy_into(),
            epsilon = 1.0e-6);

        let result: f64 = EncointerBalances::balance(cid, &get_accountid(&bob)).lossy_into();
        assert_abs_diff_eq!(
            result,
            EncointerCeremonies::ceremony_reward().lossy_into(),
            epsilon = 1.0e-6);

        assert_eq!(EncointerBalances::balance(cid, &get_accountid(&charlie)), ZERO);
        assert_eq!(EncointerBalances::balance(cid, &get_accountid(&eve)), ZERO);
        assert_eq!(EncointerBalances::balance(cid, &get_accountid(&ferdie)), ZERO);

        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &get_accountid(&alice)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &get_accountid(&bob)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &get_accountid(&charlie)),
            Reputation::Unverified
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &get_accountid(&eve)),
            Reputation::Unverified
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &get_accountid(&ferdie)),
            Reputation::Unverified
        );
    });
}

#[test]
fn bootstrapping_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = perform_bootstrapping_ceremony();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let ferdie = AccountKeyring::Ferdie.pair();

        let cindex = EncointerScheduler::current_ceremony_index();

        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &get_accountid(&alice)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &get_accountid(&bob)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation(
                (cid, cindex - 1),
                &get_accountid(&charlie)
            ),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &get_accountid(&dave)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &get_accountid(&eve)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &get_accountid(&ferdie)),
            Reputation::VerifiedUnlinked
        );
    });
}

#[test]
fn grant_reputation_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = perform_bootstrapping_ceremony();
        let master = AccountId::from(AccountKeyring::Alice);
        // a non-bootstrapper
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        assert_ok!(EncointerCeremonies::grant_reputation(
            Origin::signed(master.clone()),
            cid,
            get_accountid(&zoran)
        ));
    });
}

#[test]
fn register_with_reputation_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = perform_bootstrapping_ceremony();
        let master = AccountId::from(AccountKeyring::Alice);

        // a non-bootstrapper
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        let zoran_new = sr25519::Pair::from_entropy(&[8u8; 32], None).0;

        // another non-bootstrapper
        let yuri = sr25519::Pair::from_entropy(&[9u8; 32], None).0;

        let cindex = EncointerScheduler::current_ceremony_index();

        // fake reputation registry for first ceremony
        EncointerCeremonies::fake_reputation(
            (cid, cindex - 1),
            &get_accountid(&zoran),
            Reputation::VerifiedUnlinked,
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), get_accountid(&zoran)),
            Reputation::VerifiedUnlinked
        );

        let cindex = EncointerScheduler::current_ceremony_index();
        println!("cindex {}", cindex);
        // wrong sender of good proof fails
        let proof = prove_attendance(get_accountid(&zoran_new), cid, cindex - 1, &zoran);
        assert!(EncointerCeremonies::register_participant(
            Origin::signed(get_accountid(&yuri)),
            cid,
            Some(proof)
        )
        .is_err());

        // see if Zoran can register with his fresh key
        // for the next ceremony claiming his former attendance
        let proof = prove_attendance(get_accountid(&zoran_new), cid, cindex - 1, &zoran);
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(get_accountid(&zoran_new)),
            cid,
            Some(proof)
        ));
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), get_accountid(&zoran_new)),
            Reputation::UnverifiedReputable
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), get_accountid(&zoran)),
            Reputation::VerifiedLinked
        );

        // double signing (re-using reputation) fails
        let proof_second = prove_attendance(get_accountid(&yuri), cid, cindex - 1, &zoran);
        assert!(EncointerCeremonies::register_participant(
            Origin::signed(get_accountid(&yuri)),
            cid,
            Some(proof_second)
        )
        .is_err());

        // signer without reputation fails
        let proof = prove_attendance(get_accountid(&yuri), cid, cindex - 1, &yuri);
        assert!(EncointerCeremonies::register_participant(
            Origin::signed(get_accountid(&yuri)),
            cid,
            Some(proof)
        )
        .is_err());
    });
}

/*
#[test]
fn test_random_permutation_works() {
    ExtBuilder::build().execute_with(|| {
        let ordered = vec!(1u8, 2, 3, 4, 5, 6);
        let permutation = EncointerCeremonies::random_permutation(ordered);
        println!("random permutation result {}", permutation);
    });
}
*/

// integration tests ////////////////////////////////

#[test]
fn get_meetup_time_works() {
    ExtBuilder::build().execute_with(|| {
        System::set_block_number(0);
        run_to_block(1);

        let cid = register_test_currency();

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(EncointerScheduler::next_phase_timestamp(), 
            (GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)) + ONE_DAY);

        run_to_next_phase();

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );

        run_to_next_phase();

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ATTESTING
        );
        
        assert_eq!(EncointerCeremonies::get_meetup_time(&cid,1), 
            Some(GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 3*ONE_DAY));

        assert_eq!(EncointerCeremonies::get_meetup_time(&cid,2), 
            Some(GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 3*ONE_DAY - 1*ONE_DAY/360));

        assert_eq!(EncointerCeremonies::get_meetup_time(&cid,3), 
            Some(GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 3*ONE_DAY - 2*ONE_DAY/360));
    });
}

#[test]
fn ceremony_index_and_purging_registry_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();
        assert_ok!(EncointerCeremonies::register_participant(
            Origin::signed(alice.clone()),
            cid,
            None
        ));
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();

        // now assigning
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();
        // now attesting
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();
        // now again registering
        let new_cindex = EncointerScheduler::current_ceremony_index();
        assert_eq!(new_cindex, cindex + 1);
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 0);
        assert_eq!(
            EncointerCeremonies::participant_registry((cid, cindex), &1),
            AccountId::default()
        );
        assert_eq!(
            EncointerCeremonies::participant_index((cid, cindex), &alice),
            NONE
        );
    });
}

#[test]
fn assigning_meetup_at_phase_change_and_purge_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = register_test_currency();
        let master = AccountId::from(AccountKeyring::Alice);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        assert_eq!(
            EncointerCeremonies::meetup_index((cid, cindex), &alice),
            NONE
        );
        run_to_next_phase();
        assert_eq!(EncointerCeremonies::meetup_index((cid, cindex), &alice), 1);
        run_to_next_phase();
        run_to_next_phase();
        assert_eq!(
            EncointerCeremonies::meetup_index((cid, cindex), &alice),
            NONE
        );
    });
}

#[test]
fn grow_population_works() {
    ExtBuilder::build().execute_with(|| {
        let cid = perform_bootstrapping_ceremony();
        let cindex = EncointerScheduler::current_ceremony_index();
        // bootstrappers must register because only they have reputation now
        register_alice_bob_ferdie(cid);
        register_charlie_dave_eve(cid);
        let master = AccountId::from(AccountKeyring::Alice);
        let mut participants = Vec::<sr25519::Pair>::with_capacity(64);
        // add bootstrappers
        participants.push(AccountKeyring::Alice.pair());
        participants.push(AccountKeyring::Bob.pair());
        participants.push(AccountKeyring::Charlie.pair());
        participants.push(AccountKeyring::Dave.pair());
        participants.push(AccountKeyring::Eve.pair());
        participants.push(AccountKeyring::Ferdie.pair());

        // generate many keys and register all of them
        // they will use the same keys per participant throughout to following ceremonies
        let mut population_counter = 6u8;
        while population_counter < 20 {
            let mut entropy = [0u8; 32];
            entropy[0] = population_counter;
            let pair = sr25519::Pair::from_entropy(&entropy, None).0;
            participants.push(pair.clone());
            EncointerCeremonies::register_participant(
                Origin::signed(get_accountid(&pair)),
                cid,
                None,
            );
            population_counter += 1;
        }

        let cindex = EncointerScheduler::current_ceremony_index();
        run_to_next_phase();
        // ASSIGNING
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);
        let meetup2_1 = EncointerCeremonies::meetup_registry((cid, cindex), 1);

        // whitepaper III-B Rule 3: no more than 1/4 participants without reputation
        assert_eq!(meetup2_1.len(), 8);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetup(cid, participants.clone(), 1);

        run_to_next_phase();
        // REGISTERING

        let cindex = EncointerScheduler::current_ceremony_index();
        // register everybody again. also those who didn't have the chance last time
        for pair in participants.iter() {
            let proof = match EncointerCeremonies::participant_reputation(
                (cid, cindex - 1),
                get_accountid(&pair),
            ) {
                Reputation::VerifiedUnlinked => Some(prove_attendance(
                    get_accountid(&pair),
                    cid,
                    cindex - 1,
                    &pair,
                )),
                _ => None,
            };
            EncointerCeremonies::register_participant(
                Origin::signed(get_accountid(&pair)),
                cid,
                proof,
            );
        }
        run_to_next_phase();
        // ASSIGNING
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);
        let meetup3_1 = EncointerCeremonies::meetup_registry((cid, cindex), 1);
        // whitepaper III-B Rule 3: no more than 1/4 participants without reputation
        assert_eq!(meetup3_1.len(), 10);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetup(cid, participants.clone(), 1);

        run_to_next_phase();
        // REGISTERING

        let cindex = EncointerScheduler::current_ceremony_index();
        for pair in participants.iter() {
            let proof = match EncointerCeremonies::participant_reputation(
                (cid, cindex - 1),
                get_accountid(&pair),
            ) {
                Reputation::VerifiedUnlinked => Some(prove_attendance(
                    get_accountid(&pair),
                    cid,
                    cindex - 1,
                    &pair,
                )),
                _ => None,
            };
            EncointerCeremonies::register_participant(
                Origin::signed(get_accountid(&pair)),
                cid,
                proof,
            );
        }
        run_to_next_phase();
        // ASSIGNING
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 2);
        let meetup4_1 = EncointerCeremonies::meetup_registry((cid, cindex), 1);
        let meetup4_2 = EncointerCeremonies::meetup_registry((cid, cindex), 2);

        // whitepaper III-B Rule 3: no more than 1/4 participants without reputation
        assert!(meetup4_1.len() + meetup4_2.len() <= 13);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetup(cid, participants.clone(), 1);
        fully_attest_meetup(cid, participants.clone(), 2);

        run_to_next_phase();
        // REGISTERING

        // TODO: whitepaper III-B Rule 1: minimize the number of participants that have met at previous ceremony
        // TODO: whitepaper III-B Rule 2: maximize number of participants per meetup within 3<=N<=12
    });
}
