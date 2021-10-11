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

use super::*;
use mock::{EncointerCeremonies, EncointerBalances, EncointerScheduler, EncointerCommunities, Timestamp, Origin, new_test_ext, System, TestRuntime, TestClaim, TestProofOfAttendance};

use encointer_primitives::{
    communities::{CommunityIdentifier, Degree, Location, LossyInto},
    scheduler::{CeremonyIndexType, CeremonyPhaseType},
};
use frame_support::{
    assert_ok,
    pallet_prelude::ProvideInherent,
    traits::{OnFinalize, OnInitialize, UnfilteredDispatchable}
};
use rstest::*;
use sp_core::{sr25519, Pair, U256};
use sp_runtime::{
    DispatchError,
};
use std::ops::Rem;

use test_utils::{
    helpers::{account_id, bootstrappers, register_test_community},
    *,
};

/// Run until a particular block.
fn run_to_block(n: u64) {
    while System::block_number() < n {
        if System::block_number() > 1 {
            System::on_finalize(System::block_number());
        }
        set_timestamp(GENESIS_TIME + BLOCKTIME * n);
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

pub fn set_timestamp(t: u64) {
    let _ = <timestamp::Pallet<TestRuntime> as ProvideInherent>::Call::set(t)
        .dispatch_bypass_filter(Origin::none());
}

/// get correct meetup time for a certain cid and meetup
fn correct_meetup_time(cid: &CommunityIdentifier, mindex: MeetupIndexType) -> Moment {
    //assert_eq!(EncointerScheduler::current_phase(), CeremonyPhaseType::ATTESTING);
    let cindex = EncointerScheduler::current_ceremony_index() as u64;
    let mlon: f64 = EncointerCeremonies::get_meetup_location(cid, mindex)
        .unwrap()
        .lon
        .lossy_into();

    let t = GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)
        + cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::REGISTERING)
        + cindex * EncointerScheduler::phase_durations(CeremonyPhaseType::ASSIGNING)
        + (cindex - 1) * EncointerScheduler::phase_durations(CeremonyPhaseType::ATTESTING)
        + ONE_DAY / 2
        - (mlon / 360.0 * ONE_DAY as f64) as u64;
    t.into()
}

fn signed_claim(
    claimant: &sr25519::Pair,
    cid: CommunityIdentifier,
    cindex: CeremonyIndexType,
    mindex: MeetupIndexType,
    location: Location,
    timestamp: Moment,
    participant_count: u32,
) -> TestClaim {
    TestClaim::new_unsigned(
        claimant.public().into(),
        cindex,
        cid,
        mindex,
        location,
        timestamp,
        participant_count
    ).sign(claimant)
}

fn get_proof(
    cid: CommunityIdentifier,
    cindex: CeremonyIndexType,
    pair: &sr25519::Pair,
) -> Option<TestProofOfAttendance> {
    match EncointerCeremonies::participant_reputation((cid, cindex), account_id(pair)) {
        Reputation::VerifiedUnlinked => {
            Some(prove_attendance(account_id(&pair), cid, cindex, pair))
        }
        _ => None,
    }
}

/// generate a proof of attendance based on previous reputation
fn prove_attendance(
    prover: AccountId,
    cid: CommunityIdentifier,
    cindex: CeremonyIndexType,
    attendee: &sr25519::Pair,
) -> TestProofOfAttendance {
    let msg = (prover.clone(), cindex);
    ProofOfAttendance {
        prover_public: prover,
        community_identifier: cid,
        ceremony_index: cindex,
        attendee_public: account_id(&attendee),
        attendee_signature: Signature::from(attendee.sign(&msg.encode())),
    }
}

/// Wrapper for EncointerCeremonies::register_participant that reduces boilerplate code.
fn register(
    account: AccountId,
    cid: CommunityIdentifier,
    proof: Option<TestProofOfAttendance>,
) -> DispatchResult {
    EncointerCeremonies::register_participant(Origin::signed(account), cid, proof)
}

/// shortcut to register well-known keys for current ceremony
fn register_alice_bob_ferdie(cid: CommunityIdentifier) {
    assert_ok!(register(
        account_id(&AccountKeyring::Alice.pair()),
        cid,
        None
    ));
    assert_ok!(register(account_id(&AccountKeyring::Bob.pair()), cid, None));
    assert_ok!(register(
        account_id(&AccountKeyring::Ferdie.pair()),
        cid,
        None
    ));
}

/// shortcut to register well-known keys for current ceremony
fn register_charlie_dave_eve(cid: CommunityIdentifier) {
    assert_ok!(register(
        account_id(&AccountKeyring::Charlie.pair()),
        cid,
        None
    ));
    assert_ok!(register(
        account_id(&AccountKeyring::Dave.pair()),
        cid,
        None
    ));
    assert_ok!(register(account_id(&AccountKeyring::Eve.pair()), cid, None));
}

/// Creates new key pairs. It implicitly assumes that the i-th key was created with entropy = i.
fn add_population(amount: usize, current_popuplation_size: usize) -> Vec<sr25519::Pair> {
    let mut participants = Vec::with_capacity(amount);
    for population_counter in 1..=amount {
        let entropy = U256::from(current_popuplation_size + population_counter);
        participants.push(sr25519::Pair::from_entropy(&entropy.encode()[..], None).0);
    }
    participants
}

/// shorthand for generating multiple identical signed claims of the attestees
fn attest_all(
    attestor: AccountId,
    attestees: &Vec<&sr25519::Pair>,
    cid: CommunityIdentifier,
    cindex: CeremonyIndexType,
    mindex: MeetupIndexType,
    location: Location,
    timestamp: Moment,
    n_participants: u32,
) {
    let mut claims: Vec<TestClaim> = vec![];
    for a in attestees {
        claims.push(
            TestClaim::new_unsigned(
                a.public().into(),
                cindex,
                cid,
                mindex,
                location,
                timestamp,
                n_participants
            ).sign(*a)
        );
    }
    assert_ok!(EncointerCeremonies::attest_claims(
        Origin::signed(attestor),
        claims
    ));
}

fn attest(attestor: AccountId, claims: Vec<TestClaim>) {
    assert_ok!(EncointerCeremonies::attest_claims(
        Origin::signed(attestor),
        claims
    ));
}

/// perform bootstrapping ceremony for test community with either the supplied bootstrappers or the default bootstrappers
fn perform_bootstrapping_ceremony(
    custom_bootstrappers: Option<Vec<sr25519::Pair>>,
    n_locations: u32,
) -> CommunityIdentifier {
    let bootstrappers: Vec<sr25519::Pair> = custom_bootstrappers.unwrap_or_else(|| bootstrappers());
    let cid = register_test_community::<TestRuntime>(Some(bootstrappers.clone()), 0.0, 0.0);
    if n_locations > 70 {
        panic!("Too many locations.")
    }
    for i in 1..n_locations {
        let coord = i as f64;
        let location = Location {
            lat: Degree::from_num(coord),
            lon: Degree::from_num(coord),
        };

        match EncointerCommunities::add_location(Origin::signed(bootstrappers[0].public().into()), cid, location) {
            Ok(_v) => (),
            Err(e) => panic!("{:?}", e),
        }

    }
    bootstrappers
        .iter()
        .for_each(|b| register(b.public().into(), cid, None).unwrap());

    let cindex = EncointerScheduler::current_ceremony_index();

    run_to_next_phase();
    // ASSIGNING
    run_to_next_phase();
    // ATTESTING
    let loc = EncointerCommunities::get_locations(&cid)[0];
    let time = correct_meetup_time(&cid, 0);

    for i in 0..bootstrappers.len() {
        let mut bs = bootstrappers.clone();
        let claimant = bs.remove(i);
        attest_all(account_id(&claimant), &bs.iter().collect(), cid, cindex, 0, loc, time, 6);
    }
    run_to_next_phase();
    // REGISTERING
    cid
}

/// perform full attestation of all participants for a given meetup
fn fully_attest_meetup(
    cid: CommunityIdentifier,
    keys: Vec<sr25519::Pair>,
    mindex: MeetupIndexType,
) {
    let cindex = EncointerScheduler::current_ceremony_index();
    let meetup = EncointerCeremonies::get_meetup_participants((cid, cindex), mindex);
    for p in meetup.iter() {
        let mut others = Vec::with_capacity(meetup.len() - 1);
        for o in meetup.iter() {
            if o == p {
                continue;
            }
            for pair in keys.iter() {
                if account_id(pair) == *o {
                    others.push(pair.clone());
                }
            }
        }
        let loc  = EncointerCommunities::get_locations(&cid)[(mindex) as usize];
        let time = correct_meetup_time(&cid, mindex);

        attest_all(
            (*p).clone(),
            &others.iter().collect(),
            cid,
            cindex,
            mindex,
            loc,
            time,
            meetup.len() as u32,
        );
    }
}

fn assert_error(actual: DispatchResult, expected: Error::<TestRuntime>) {
    assert_eq!(match actual.clone().err().unwrap() {
        sp_runtime::DispatchError::Module { index, error, message } => message,
        _ => panic!(),
    }.unwrap(), expected.as_str());
}

// unit tests ////////////////////////////////////////

#[test]
fn registering_participant_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let cindex = EncointerScheduler::current_ceremony_index();

        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 0);
        assert_ok!(register(alice.clone(), cid, None));

        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 1);
        assert_ok!(register(bob.clone(), cid, None));

        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 2);

        assert_eq!(
            EncointerCeremonies::bootstrapper_index((cid, cindex), &bob),
            2
        );
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &1),
            alice
        );
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &2),
            bob
        );


        let newbies = add_population(2, 2);
        let newbie_1 = account_id(&newbies[0]);
        let newbie_2 = account_id(&newbies[01]);
        assert_ok!(register(newbie_1.clone(), cid, None));
        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 1);


        assert_ok!(register(newbie_2.clone(), cid, None));
        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 2);
        assert_eq!(
            EncointerCeremonies::newbie_index((cid, cindex), &newbie_1),
            1
        );
        assert_eq!(
            EncointerCeremonies::newbie_registry((cid, cindex), &1),
            newbie_1
        );

        assert_eq!(
            EncointerCeremonies::newbie_index((cid, cindex), &newbie_2),
            2
        );
        assert_eq!(
            EncointerCeremonies::newbie_registry((cid, cindex), &2),
            newbie_2
        );


        let newbies = add_population(2, 4);
        let endorsee_1 = account_id(&newbies[0]);
        let endorsee_2 = account_id(&newbies[1]);
        assert_ok!(EncointerCeremonies::endorse_newcomer(
                Origin::signed(alice.clone()),
                cid,
                endorsee_1.clone())
            );

        assert_ok!(EncointerCeremonies::endorse_newcomer(
                Origin::signed(alice.clone()),
                cid,
                endorsee_2.clone())
            );

        assert_ok!(register(endorsee_1.clone(), cid, None));
        assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 1);


        assert_ok!(register(endorsee_2.clone(), cid, None));
        assert_eq!(EncointerCeremonies::endorsee_count((cid, cindex)), 2);

        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 2);
        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 2);

        assert_eq!(
            EncointerCeremonies::endorsee_index((cid, cindex), &endorsee_1),
            1
        );
        assert_eq!(
            EncointerCeremonies::endorsee_registry((cid, cindex), &1),
            endorsee_1
        );

        assert_eq!(
            EncointerCeremonies::endorsee_index((cid, cindex), &endorsee_2),
            2
        );
        assert_eq!(
            EncointerCeremonies::endorsee_registry((cid, cindex), &2),
            endorsee_2
        );

        // Registering Reputables is tested in grow_population_works.

    });
}

#[test]
fn registering_participant_twice_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        assert_ok!(register(alice.clone(), cid, None));
        assert!(register(alice.clone(), cid, None).is_err());
    });
}

#[test]
fn registering_participant_in_wrong_phase_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        run_to_next_phase();
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );
        assert!(register(alice.clone(), cid, None).is_err());
    });
}

#[test]
fn attest_claims_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        assert_eq!(
            EncointerCeremonies::get_meetup_index((cid, cindex), &account_id(&alice)).unwrap(),
            0
        );
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);
        attest_all(
            account_id(&alice),
            &vec![&bob, &ferdie],
            cid,
            1,
            0,
            loc,
            time,
            3,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice, &ferdie],
            cid,
            1,
            0,
            loc,
            time,
            3,
        );

        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
        assert_eq!(
            EncointerCeremonies::attestation_index((cid, cindex), &account_id(&bob)),
            2
        );
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &2);
        assert!(wit_vec.len() == 2);
        assert!(wit_vec.contains(&account_id(&alice)));
        assert!(wit_vec.contains(&account_id(&ferdie)));

        // TEST: re-registering must overwrite previous entry
        attest_all(
            account_id(&alice),
            &vec![&bob, &ferdie],
            cid,
            1,
            0,
            loc,
            time,
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 2);
    });
}

#[test]
fn attest_claims_for_non_participant_fails_silently() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING

        attest_all(
            account_id(&alice),
            &vec![&bob, &alice],
            cid,
            1,
            0,
            Location::default(),
            correct_meetup_time(&cid, 0),
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&account_id(&alice)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn attest_claims_for_non_participant_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let mut eve_claims: Vec<TestClaim> = vec![];
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);
        eve_claims.insert(
            0,
            signed_claim(
                &alice,
                cid,
                cindex,
                0,
                loc,
                time,
                3,
            ),
        );
        eve_claims.insert(
            1,
            signed_claim(
                &ferdie,
                cid,
                cindex,
                0,
                loc,
                time,
                3,
            ),
        );
        assert!(EncointerCeremonies::attest_claims(
            Origin::signed(account_id(&eve)),
            eve_claims.clone()
        )
        .is_err());
    });
}

#[test]
fn attest_claims_with_non_participant_fails_silently() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let eve = AccountKeyring::Eve.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        attest_all(
            account_id(&alice),
            &vec![&bob, &eve],
            cid,
            1,
            0,
            Location::default(),
            correct_meetup_time(&cid, 0),
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&account_id(&eve)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn attest_claims_with_wrong_meetup_index_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);
        let mut alice_claims: Vec<TestClaim> = vec![];
        alice_claims.push(
            signed_claim(&bob, cid, 1, 0, loc, time, 3),
        );
        let bogus_claim = signed_claim(&ferdie, cid, 1,
                                       1 + 99,
                                       // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
                                       Location::default(),
                                       time,
                                       3,
        );
        alice_claims.push(
            bogus_claim
        );
        assert_ok!(EncointerCeremonies::attest_claims(
            Origin::signed(account_id(&alice)),
            alice_claims
        ));
        let attestees = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(attestees.contains(&account_id(&ferdie)) == false);
        assert!(attestees.len() == 1);
    });
}

#[test]
fn attest_claims_with_wrong_ceremony_index_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);
        let mut alice_attestations: Vec<TestClaim> = vec![];
        alice_attestations.push(
            signed_claim(&bob, cid, 1, 0, loc, time, 3),
        );
        let bogus_claim = signed_claim(
            &ferdie,
            cid,
            // !!!!!!!!!!!!!!!!!!!!!!!!!!
            99,
            1,
            Location::default(),
            time,
            3,
        );
        alice_attestations.push(
            bogus_claim
        );
        assert_ok!(EncointerCeremonies::attest_claims(
            Origin::signed(account_id(&alice)),
            alice_attestations
        ));
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&account_id(&ferdie)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn attest_claims_with_wrong_timestamp_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        run_to_next_phase();
        run_to_next_phase();
        // ATTESTING
        let loc = Location {
            lon: Degree::from_num(25.9),
            lat: Degree::from_num(0),
        };
        // too late!
        let time = correct_meetup_time(&cid, 0) + TIME_TOLERANCE + 1;
        let mut alice_claims: Vec<TestClaim> = vec![];
        alice_claims.push(signed_claim(
            &bob,
            cid,
            1,
            1,
            loc,
            time,
            3,
        ));
        alice_claims.push(signed_claim(
            &ferdie,
            cid,
            1,
            1,
            loc,
            time,
            3,
        ));
        assert!(EncointerCeremonies::attest_claims(
            Origin::signed(account_id(&alice)),
            alice_claims
        )
        .is_err());
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.len() == 0);
    });
}

#[test]
fn attest_claims_with_wrong_location_fails() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
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
        let time = correct_meetup_time(&cid, 0);
        let mut alice_claims: Vec<TestClaim> = vec![];
        alice_claims.push(signed_claim(
            &bob,
            cid,
            1,
            1,
            loc,
            time,
            3,
        ));
        alice_claims.push(signed_claim(
            &ferdie,
            cid,
            1,
            1,
            loc,
            time,
            3,
        ));
        assert!(EncointerCeremonies::attest_claims(
            Origin::signed(account_id(&alice)),
            alice_claims
        )
        .is_err());
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.len() == 0);
    });
}

#[test]
fn ballot_meetup_n_votes_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        register_charlie_dave_eve(cid);

        run_to_next_phase();
        // ASSIGNING
        run_to_next_phase();
        // ATTESTING
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);
        attest_all(
            account_id(&alice),
            &vec![&bob, &charlie, &dave, &eve, &ferdie],
            cid,
            cindex,
            0,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&ferdie),
            &vec![&alice],
            cid,
            cindex,
            0,
            loc,
            time,
            6,
        );
        // assert that majority vote was successful
        assert_eq!(
            EncointerCeremonies::ballot_meetup_n_votes(&cid, cindex, 0),
            Some((5, 5))
        );

        attest_all(
            account_id(&alice),
            &vec![&bob],
            cid,
            1,
            0,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice],
            cid,
            1,
            0,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&alice),
            &vec![&charlie, &dave],
            cid,
            1,
            0,
            loc,
            time,
            4,
        );
        attest_all(
            account_id(&alice),
            &vec![&eve, &ferdie],
            cid,
            1,
            0,
            loc,
            time,
            6,
        );
        // votes should be (4, 2), (5, 2), (6, 2)
        assert!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 0) == None);

        attest_all(
            account_id(&alice),
            &vec![&bob, &charlie],
            cid,
            1,
            0,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice],
            cid,
            1,
            0,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&alice),
            &vec![&dave],
            cid,
            1,
            0,
            loc,
            time,
            4,
        );
        attest_all(
            account_id(&alice),
            &vec![&eve, &ferdie],
            cid,
            1,
            0,
            loc,
            time,
            6,
        );
        // votes should be (5, 3), (6, 2), (4, 1)
        assert_eq!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 0), Some((5, 3)));
    });
}

#[test]
fn issue_reward_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let ferdie = AccountKeyring::Ferdie.pair();
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        register_charlie_dave_eve(cid);

        let loc = Location::default();
        let time = correct_meetup_time(&cid, 0);

        let claim_base = TestClaim::new_unsigned(
            account_id(&alice),
            cindex,
            cid,
            0,
            loc,
            time,
            5,
        );

        let claim_alice = claim_base.clone().sign(&alice);
        let claim_bob = claim_base.clone().set_claimant(account_id(&bob)).sign(&bob);
        let claim_charlie = claim_base.clone().set_claimant(account_id(&charlie)).sign(&alice);
        let claim_dave = claim_base.clone()
            .set_claimant(account_id(&dave))
            .set_participant_count(6)
            .sign(&dave);
        let claim_eve = claim_base.clone().set_claimant(account_id(&eve)).sign(&eve);
        let claim_ferdie = claim_base.clone().set_claimant(account_id(&ferdie)).sign(&ferdie);

        run_to_next_phase();
        // ASSIGNING
        run_to_next_phase();
        // ATTESTING
        // Scenario:
        //      ferdie doesn't show up
        //      eve signs no one else
        //      charlie collects bogus signatures
        //      dave signs ferdie and reports wrong number of participants

        // alice attests all others except for ferdie, who doesn't show up
        attest(account_id(&alice), vec![claim_bob.clone(), claim_charlie.clone(), claim_dave.clone(), claim_eve.clone()]);
        // bob attests all others except for ferdie, who doesn't show up
        attest(account_id(&bob), vec![claim_alice.clone(), claim_charlie.clone(), claim_dave.clone(), claim_eve.clone()]);
        // charlie attests all others except for ferdie, who doesn't show up, but he supplies erroneous signatures with the others' claims
        attest(account_id(&charlie), vec![claim_alice.clone(), claim_bob.clone(), claim_dave.clone(), claim_eve.clone()]);
        // dave attests all others plus nonexistent ferdie and reports wrong number
        attest(account_id(&dave), vec![claim_alice.clone(), claim_bob.clone(), claim_charlie.clone(), claim_eve.clone(), claim_ferdie.clone()]);
        // eve does not attest anybody...
        // ferdie is not here...

        assert_eq!(EncointerBalances::balance(cid, &account_id(&alice)), ZERO);

        run_to_next_phase();
        // REGISTERING

        let result: f64 = EncointerBalances::balance(cid, &account_id(&alice)).lossy_into();
        assert_abs_diff_eq!(
            result,
            EncointerCeremonies::ceremony_reward().lossy_into(),
            epsilon = 1.0e-6
        );

        let result: f64 = EncointerBalances::balance(cid, &account_id(&bob)).lossy_into();
        assert_abs_diff_eq!(
            result,
            EncointerCeremonies::ceremony_reward().lossy_into(),
            epsilon = 1.0e-6
        );

        assert_eq!(EncointerBalances::balance(cid, &account_id(&charlie)), ZERO);
        assert_eq!(EncointerBalances::balance(cid, &account_id(&eve)), ZERO);
        assert_eq!(EncointerBalances::balance(cid, &account_id(&ferdie)), ZERO);

        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&alice)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&bob)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&charlie)),
            Reputation::Unverified
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&eve)),
            Reputation::Unverified
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), &account_id(&ferdie)),
            Reputation::Unverified
        );
    });
}

#[test]
fn bootstrapping_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let alice = AccountKeyring::Alice.pair();
        let bob = AccountKeyring::Bob.pair();
        let charlie = AccountKeyring::Charlie.pair();
        let dave = AccountKeyring::Dave.pair();
        let eve = AccountKeyring::Eve.pair();
        let ferdie = AccountKeyring::Ferdie.pair();

        let cindex = EncointerScheduler::current_ceremony_index();

        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&alice)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&bob)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&charlie)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&dave)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&eve)),
            Reputation::VerifiedUnlinked
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), &account_id(&ferdie)),
            Reputation::VerifiedUnlinked
        );
    });
}

#[test]
fn grant_reputation_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let master = AccountId::from(AccountKeyring::Alice);
        // a non-bootstrapper
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        assert_ok!(EncointerCeremonies::grant_reputation(
            Origin::signed(master.clone()),
            cid,
            account_id(&zoran)
        ));
    });
}

#[test]
fn register_with_reputation_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);

        // a non-bootstrapper
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        let zoran_new = sr25519::Pair::from_entropy(&[8u8; 32], None).0;

        // another non-bootstrapper
        let yuri = sr25519::Pair::from_entropy(&[9u8; 32], None).0;

        let cindex = EncointerScheduler::current_ceremony_index();

        // fake reputation registry for first ceremony
        EncointerCeremonies::fake_reputation(
            (cid, cindex - 1),
            &account_id(&zoran),
            Reputation::VerifiedUnlinked,
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), account_id(&zoran)),
            Reputation::VerifiedUnlinked
        );

        let cindex = EncointerScheduler::current_ceremony_index();
        println!("cindex {}", cindex);
        // wrong sender of good proof fails
        let proof = prove_attendance(account_id(&zoran_new), cid, cindex - 1, &zoran);
        assert!(register(account_id(&yuri), cid, Some(proof)).is_err());

        // see if Zoran can register with his fresh key
        // for the next ceremony claiming his former attendance
        let proof = prove_attendance(account_id(&zoran_new), cid, cindex - 1, &zoran);
        assert_ok!(register(account_id(&zoran_new), cid, Some(proof)));
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex), account_id(&zoran_new)),
            Reputation::UnverifiedReputable
        );
        assert_eq!(
            EncointerCeremonies::participant_reputation((cid, cindex - 1), account_id(&zoran)),
            Reputation::VerifiedLinked
        );

        // double signing (re-using reputation) fails
        let proof_second = prove_attendance(account_id(&yuri), cid, cindex - 1, &zoran);
        assert!(register(account_id(&yuri), cid, Some(proof_second)).is_err());

        // signer without reputation fails
        let proof = prove_attendance(account_id(&yuri), cid, cindex - 1, &yuri);
        assert!(register(account_id(&yuri), cid, Some(proof)).is_err());
    });
}

#[test]
fn endorsing_newbie_works_until_no_more_tickets() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let alice = AccountId::from(AccountKeyring::Alice);

        let endorsees = add_population((AMOUNT_NEWBIE_TICKETS + 1) as usize, 6);
        for i in 0..AMOUNT_NEWBIE_TICKETS {
            assert_ok!(EncointerCeremonies::endorse_newcomer(
                Origin::signed(alice.clone()),
                cid,
                account_id(&endorsees[i as usize])
            ));
        }

        assert_error(
            EncointerCeremonies::endorse_newcomer(
                Origin::signed(alice.clone()),
                cid,
                account_id(&endorsees[AMOUNT_NEWBIE_TICKETS as usize]),
            ),
            Error::<TestRuntime>::NoMoreNewbieTickets,
        );
    });
}

#[test]
fn endorsing_newbie_for_second_next_ceremony_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();
        run_to_next_phase();

        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::ASSIGNING
        );
        // a newbie
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        assert_ok!(EncointerCeremonies::endorse_newcomer(
            Origin::signed(alice.clone()),
            cid,
            account_id(&zoran)
        ));
        assert!(Endorsees::<TestRuntime>::contains_key(
            (cid, cindex + 1),
            &account_id(&zoran)
        ));
    });
}

#[test]
fn endorsing_newbie_twice_fails() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();

        // a newbie
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        assert_ok!(EncointerCeremonies::endorse_newcomer(
            Origin::signed(alice.clone()),
            cid,
            account_id(&zoran)
        ));
        assert!(Endorsees::<TestRuntime>::contains_key(
            (cid, cindex),
            &account_id(&zoran)
        ));
        assert_error(
            EncointerCeremonies::endorse_newcomer(
                Origin::signed(alice.clone()),
                cid,
                account_id(&zoran),
            ),
            Error::<TestRuntime>::AlreadyEndorsed
        );
    });
}

#[test]
fn endorsing_two_newbies_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();

        // a newbie
        let yran = sr25519::Pair::from_entropy(&[8u8; 32], None).0;
        let zoran = sr25519::Pair::from_entropy(&[9u8; 32], None).0;
        assert_ok!(EncointerCeremonies::endorse_newcomer(
            Origin::signed(alice.clone()),
            cid,
            account_id(&zoran)
        ));
        assert!(Endorsees::<TestRuntime>::contains_key(
            (cid, cindex),
            &account_id(&zoran)
        ));
        assert_ok!(EncointerCeremonies::endorse_newcomer(
            Origin::signed(alice.clone()),
            cid,
            account_id(&yran)
        ));
        assert!(Endorsees::<TestRuntime>::contains_key(
            (cid, cindex),
            &account_id(&yran)
        ));
    });
}

// integration tests ////////////////////////////////

#[rstest(lat_micro, lon_micro,
case(0, 0),
case(1_000_000, 1_000_000),
case(0, 2_234_567),
case(2_000_000, 155_000_000),
case(1_000_000, -2_000_000),
case(-31_000_000, -155_000_000),
)]
fn get_meetup_time_works(lat_micro: i64, lon_micro: i64) {
    new_test_ext().execute_with(|| {
        System::set_block_number(0);
        run_to_block(1);

        let cid = register_test_community::<TestRuntime>(None, lat_micro as f64 / 1_000_000.0, lon_micro as f64 / 1_000_000.0);
        // locations will not generally be returned in the order they were registered
        // and meetups will be at randomized locations after https://github.com/encointer/pallets/issues/65
        // that would break this test if we had more than one location registered

        assert_eq!(EncointerScheduler::current_ceremony_index(), 1);
        assert_eq!(
            EncointerScheduler::current_phase(),
            CeremonyPhaseType::REGISTERING
        );
        assert_eq!(
            EncointerScheduler::next_phase_timestamp(),
            (GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY)) + ONE_DAY
        );

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

        let mtime = if lon_micro >= 0 {
            GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 2 * ONE_DAY + ONE_DAY / 2
               - (lon_micro * ONE_DAY as i64 / 360_000_000) as u64
        } else {
            GENESIS_TIME - GENESIS_TIME.rem(ONE_DAY) + 2 * ONE_DAY + ONE_DAY / 2
                + (lon_micro.abs() * ONE_DAY as i64 / 360_000_000) as u64
        };

        let tol = 60_000; // [ms]
        assert!(tol >
            (EncointerCeremonies::get_meetup_time(&cid, 0).unwrap() as i64 -
            mtime as i64).abs() as u64
        );
    });
}

#[test]
fn ceremony_index_and_purging_registry_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();

        assert_ok!(register(alice.clone(), cid, None));
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();

        // now assigning
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();
        // now attesting
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &1),
            alice
        );
        run_to_next_phase();
        // now again registering
        let new_cindex = EncointerScheduler::current_ceremony_index();
        assert_eq!(new_cindex, cindex + 1);
        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 0);
        assert_eq!(
            EncointerCeremonies::bootstrapper_registry((cid, cindex), &1),
            AccountId::default()
        );
        assert_eq!(
            EncointerCeremonies::bootstrapper_index((cid, cindex), &alice),
            NONE
        );
    });
}

#[test]
fn grow_population_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 3);
        let mut participants = bootstrappers();

        // generate many keys and register all of them
        // they will use the same keys per participant throughout to following ceremonies
        participants.extend(add_population(14, participants.len()));
        participants
            .iter()
            .for_each(|p| register(account_id(&p), cid, None).unwrap());

        let cindex = EncointerScheduler::current_ceremony_index();
        run_to_next_phase();
        // ASSIGNING
        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
        assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 0);
        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 14);
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);

        run_to_next_phase();
        // WITNESSING

        fully_attest_meetup(cid, participants.clone(), 0);


        run_to_next_phase();
        // REGISTERING

        let cindex = EncointerScheduler::current_ceremony_index();
        // register everybody again. also those who didn't have the chance last time
        for pair in participants.iter() {
            let proof = get_proof(cid, cindex - 1, pair);
            register(account_id(&pair), cid, proof).unwrap();
        }
        run_to_next_phase();
        // ASSIGNING

        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
        assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 2);
        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 12);
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 1);

        run_to_next_phase();

        fully_attest_meetup(cid, participants.clone(), 0);

        run_to_next_phase();
        // REGISTERING

        let cindex = EncointerScheduler::current_ceremony_index();
        // register everybody again. also those who didn't have the chance last time
        for pair in participants.iter() {
            let proof = get_proof(cid, cindex - 1, pair);
            register(account_id(&pair), cid, proof).unwrap();
        }
        run_to_next_phase();
        // ASSIGNING

        assert_eq!(EncointerCeremonies::bootstrapper_count((cid, cindex)), 6);
        assert_eq!(EncointerCeremonies::reputable_count((cid, cindex)), 4);
        assert_eq!(EncointerCeremonies::newbie_count((cid, cindex)), 10);
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 2);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetup(cid, participants.clone(), 0);
        fully_attest_meetup(cid, participants.clone(), 1);

        run_to_next_phase();
        // REGISTERING
        let cindex = EncointerScheduler::current_ceremony_index();
        let mut proof_count = 0;
        for pair in participants.iter() {
            let proof = get_proof(cid, cindex - 1, &pair);
            if proof.is_some() {
                proof_count += 1;
            }
            register(account_id(&pair), cid, proof).unwrap();
        }
        run_to_next_phase();
        // ASSIGNING
        assert_eq!(proof_count, 13);
        assert_eq!(EncointerCeremonies::meetup_count((cid, cindex)), 2);
    });
}

#[test]
fn is_prime_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(EncointerCeremonies::is_prime(0), false);
        assert_eq!(EncointerCeremonies::is_prime(1), false);
        assert_eq!(EncointerCeremonies::is_prime(2), true);
        assert_eq!(EncointerCeremonies::is_prime(3), true);
        assert_eq!(EncointerCeremonies::is_prime(113), true);
        assert_eq!(EncointerCeremonies::is_prime(114), false);
        assert_eq!(EncointerCeremonies::is_prime(115), false);
    });
}

#[test]
fn find_prime_below_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(EncointerCeremonies::find_prime_below(0), 2);
        assert_eq!(EncointerCeremonies::find_prime_below(1), 2);
        assert_eq!(EncointerCeremonies::find_prime_below(1), 2);
        assert_eq!(EncointerCeremonies::find_prime_below(5), 5);
        assert_eq!(EncointerCeremonies::find_prime_below(10), 7);
        assert_eq!(EncointerCeremonies::find_prime_below(118), 113);
        assert_eq!(EncointerCeremonies::find_prime_below(113), 113);

    });
}


#[test]
fn mod_inv_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(EncointerCeremonies::mod_inv(2, 7), 4);
        assert_eq!(EncointerCeremonies::mod_inv(69, 113), 95);
        assert_eq!(EncointerCeremonies::mod_inv(111, 113), 56);
    });
}


#[test]
fn validate_equal_mapping_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(EncointerCeremonies::validate_equal_mapping(2761, 2753, 427, 2326, 1099), false);
        assert_eq!(EncointerCeremonies::validate_equal_mapping(2761, 2753, 427, 2325, 1099), true);
    });
}

#[test]
fn get_assignment_params_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let cindex = EncointerScheduler::current_ceremony_index();

        let assignment_params_bootstrappers_reputables = EncointerCeremonies::assignment_params_bootstrappers_reputables((cid, cindex));
        let assignment_params_endrosees = EncointerCeremonies::assignment_params_endorsees((cid, cindex));
        let assignment_params_newbies = EncointerCeremonies::assignment_params_newbies((cid, cindex));

        assert_eq!(assignment_params_bootstrappers_reputables.m, 0);
        assert_eq!(assignment_params_bootstrappers_reputables.s1, 0);
        assert_eq!(assignment_params_bootstrappers_reputables.s2, 0);
        assert_eq!(assignment_params_endrosees.m, 0);
        assert_eq!(assignment_params_endrosees.s1, 0);
        assert_eq!(assignment_params_endrosees.s2, 0);
        assert_eq!(assignment_params_newbies.m, 0);
        assert_eq!(assignment_params_newbies.s1, 0);
        assert_eq!(assignment_params_newbies.s2, 0);

        run_to_next_phase();

        let assignment_params_bootstrappers_reputables = EncointerCeremonies::assignment_params_bootstrappers_reputables((cid, cindex));
        let assignment_params_endrosees = EncointerCeremonies::assignment_params_endorsees((cid, cindex));
        let assignment_params_newbies = EncointerCeremonies::assignment_params_newbies((cid, cindex));

        assert!(assignment_params_bootstrappers_reputables.m >  0);
        assert!(assignment_params_bootstrappers_reputables.s1 > 0);
        assert!(assignment_params_bootstrappers_reputables.s2 > 0);
        assert!(assignment_params_endrosees.m > 0);
        assert!(assignment_params_endrosees.s1 > 0);
        assert!(assignment_params_endrosees.s2 > 0);
        assert!(assignment_params_newbies.m > 0);
        assert!(assignment_params_newbies.s1 > 0);
        assert!(assignment_params_newbies.s2 > 0);


    });
}


#[test]
fn assignment_fn_works() {
    new_test_ext().execute_with(|| {
        assert_eq!(EncointerCeremonies::assignment_fn(6, 4, 5, 5, 3), 1)
    });
}

fn check_assignment(num_participants: u64, m: u64, n: u64, s1: u64, s2:u64) {
    let mut locations: Vec<u64>= vec![0; num_participants as usize];

    for i in 0..num_participants{
        locations[i as usize] = EncointerCeremonies::assignment_fn(i, s1, s2, m, n);
    }

    let mut assigned_participants: Vec<bool>= vec![false; num_participants as usize];

    // inverse function yields the same result
    for i in 0..n {
        let participants = EncointerCeremonies::assignment_fn_inverse(i, s1, s2, m, n, num_participants);
        for p in participants {
            assigned_participants[p as usize] = true;
            assert_eq!(locations[p as usize], i)
        }
    }

    // all participants were assigned
    for val in assigned_participants{
        assert!(val);
    }
}
#[test]
fn assignment_fn_inverse_works() {
    new_test_ext().execute_with(|| {
        let mut s1 = 78u64;
        let mut s2 = 23u64;
        let mut n = 12u64;
        let mut num_participants = 118u64;
        let mut m = 113u64;
        check_assignment(num_participants, m, n, s1, s2);

         s1 = 1u64;
         s2 = 1u64;
         n = 2u64;
         num_participants = 20u64;
         m = 19u64;
        check_assignment(num_participants, m, n, s1, s2);

        s1 = 1u64;
        s2 = 1u64;
        n = 1u64;
        num_participants = 10u64;
        m = 7u64;
        check_assignment(num_participants, m, n, s1, s2);

    });
}

#[test]
fn get_meetup_index_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let cindex = EncointerScheduler::current_ceremony_index();

        let participants = add_population(4, 0);
        let p1 = account_id(&participants[0]);
        let p2 = account_id(&participants[1]);
        let p3 = account_id(&participants[2]);
        let p4 = account_id(&participants[3]);

        MeetupCount::insert((cid, cindex), 10);

        BootstrapperIndex::<TestRuntime>::insert((cid, cindex), p1.clone(), 1);
        AssignedBootstrapperCount::insert((cid, cindex), 1);

        ReputableIndex::<TestRuntime>::insert((cid, cindex), p2.clone(), 1);

        EndorseeIndex::<TestRuntime>::insert((cid, cindex), p3.clone(), 3);
        NewbieIndex::<TestRuntime>::insert((cid, cindex), p4.clone(), 4);

        let assignment_params_bootstrappers_reputables = AssignmentParams {
            m: 2,
            s1: 1,
            s2: 1,
        };

        let assignment_params_endorsees = AssignmentParams {
            m: 5,
            s1: 2,
            s2: 3,
        };

        let assignment_params_newbies = AssignmentParams {
            m: 5,
            s1: 2,
            s2: 3,
        };

        AssignmentParamsBootstrappersReputables::insert((cid, cindex), assignment_params_bootstrappers_reputables);
        AssignmentParamsEndorsees::insert((cid, cindex), assignment_params_endorsees);
        AssignmentParamsNewbies::insert((cid, cindex), assignment_params_newbies);

        assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p1).unwrap(), 1);
        assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p2).unwrap(), 0);
        assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p3).unwrap(), 2);
        assert_eq!(EncointerCeremonies::get_meetup_index((cid, cindex), &p4).unwrap(), 4);

    });
}

#[test]
fn get_meetup_participants_works() {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, 1);
        let cindex = EncointerScheduler::current_ceremony_index();

        let participants: Vec<AccountId> = add_population(12, 0).iter()
            .map(|b| account_id(&b)).collect();

        BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[0].clone());
        BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[1].clone());
        BootstrapperRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[2].clone());
        AssignedBootstrapperCount::insert((cid, cindex), 3);


        ReputableRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[3].clone());
        ReputableRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[4].clone());
        ReputableRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[5].clone());
        AssignedReputableCount::insert((cid, cindex), 3);

        EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[6].clone());
        EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[7].clone());
        EndorseeRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[8].clone());
        AssignedEndorseeCount::insert((cid, cindex), 3);

        NewbieRegistry::<TestRuntime>::insert((cid, cindex), 1, participants[9].clone());
        NewbieRegistry::<TestRuntime>::insert((cid, cindex), 2, participants[10].clone());
        NewbieRegistry::<TestRuntime>::insert((cid, cindex), 3, participants[11].clone());
        AssignedNewbieCount::insert((cid, cindex), 3);



        MeetupCount::insert((cid, cindex), 2);

        let assignment_params_bootstrappers_reputables = AssignmentParams {
            m: 5,
            s1: 2,
            s2: 3,
        };

        let assignment_params_endorsees = AssignmentParams {
            m: 3,
            s1: 2,
            s2: 1,
        };

        let assignment_params_newbies = AssignmentParams {
            m: 3,
            s1: 1,
            s2: 2,
        };

        AssignmentParamsBootstrappersReputables::insert((cid, cindex), assignment_params_bootstrappers_reputables);
        AssignmentParamsEndorsees::insert((cid, cindex), assignment_params_endorsees);
        AssignmentParamsNewbies::insert((cid, cindex), assignment_params_newbies);



        let mut m0_expected_participants = [participants[1].clone(), participants[2].clone(), participants[3].clone(), participants[7].clone(), participants[8].clone(), participants[9].clone(), participants[10].clone()];
        let mut m1_expected_participants = [participants[0].clone(), participants[4].clone(), participants[5].clone(), participants[6].clone(), participants[11].clone()];
        let mut m0_participants = EncointerCeremonies::get_meetup_participants((cid, cindex), 0);
        let mut m1_participants = EncointerCeremonies::get_meetup_participants((cid, cindex), 1);

        m0_expected_participants.sort();
        m1_expected_participants.sort();
        m0_participants.sort();
        m1_participants.sort();

        assert_eq!(m0_participants, m0_expected_participants);
        assert_eq!(m1_participants, m1_expected_participants);
    });
}

#[rstest(n_locations, n_bootstrappers, n_reputables, n_endorsees, n_newbies, exp_m_bootstrappers_reputables, exp_m_endorsees, exp_m_newbies, exp_n_assigned_bootstrappers, exp_n_assigned_reputables, exp_n_assigned_endorsees, exp_n_assigned_newbies,
case(3,7,12,6,13,19,5,5,7,12,6,5),
case(10,1,1,20,13,2,17,2,1,1,18,0),
)]
fn generate_meetup_assignment_params_works(n_locations: u64, n_bootstrappers: u64, n_reputables: u64, n_endorsees: u64, n_newbies: u64, exp_m_bootstrappers_reputables: u64, exp_m_endorsees: u64, exp_m_newbies: u64, exp_n_assigned_bootstrappers: u64, exp_n_assigned_reputables: u64, exp_n_assigned_endorsees: u64, exp_n_assigned_newbies: u64) {
    new_test_ext().execute_with(|| {
        let cid = perform_bootstrapping_ceremony(None, n_locations as u32);
        let cindex = EncointerScheduler::current_ceremony_index();
        BootstrapperCount::insert((cid, cindex), n_bootstrappers);
        ReputableCount::insert((cid, cindex), n_reputables);
        EndorseeCount::insert((cid, cindex), n_endorsees);
        NewbieCount::insert((cid, cindex), n_newbies);

        EncointerCeremonies::generate_meetup_assignment_params((cid, cindex)).ok();

        assert_eq!(EncointerCeremonies::assigned_bootstrapper_count((cid, cindex)), exp_n_assigned_bootstrappers);
        assert_eq!(EncointerCeremonies::assigned_reputable_count((cid, cindex)), exp_n_assigned_reputables);
        assert_eq!(EncointerCeremonies::assigned_endorsee_count((cid, cindex)), exp_n_assigned_endorsees);
        assert_eq!(EncointerCeremonies::assigned_newbie_count((cid, cindex)), exp_n_assigned_newbies);

        let assignment_params_bootstrappers_reputables = EncointerCeremonies::assignment_params_bootstrappers_reputables((cid, cindex));
        let assignment_params_endrosees = EncointerCeremonies::assignment_params_endorsees((cid, cindex));
        let assignment_params_newbies = EncointerCeremonies::assignment_params_newbies((cid, cindex));

        assert_eq!(assignment_params_bootstrappers_reputables.m, exp_m_bootstrappers_reputables);
        assert!(assignment_params_bootstrappers_reputables.s1 > 0);
        assert!(assignment_params_bootstrappers_reputables.s1 < exp_m_bootstrappers_reputables);
        assert!(assignment_params_bootstrappers_reputables.s2 > 0);
        assert!(assignment_params_bootstrappers_reputables.s2 < exp_m_bootstrappers_reputables);

        assert_eq!(assignment_params_endrosees.m, exp_m_endorsees);
        assert!(assignment_params_endrosees.s1 > 0);
        assert!(assignment_params_endrosees.s1 < exp_m_endorsees);
        assert!(assignment_params_endrosees.s2 > 0);
        assert!(assignment_params_endrosees.s2 < exp_m_endorsees);

        assert_eq!(assignment_params_newbies.m, exp_m_newbies);
        assert!(assignment_params_newbies.s1 > 0);
        assert!(assignment_params_newbies.s1 < exp_m_newbies);
        assert!(assignment_params_newbies.s2 > 0);
        assert!(assignment_params_newbies.s2 < exp_m_newbies);
    });
}