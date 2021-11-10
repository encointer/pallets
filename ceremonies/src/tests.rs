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

use approx::assert_abs_diff_eq;
use encointer_primitives::{
    communities::{CommunityIdentifier, Degree, Location, LossyInto},
    scheduler::{CeremonyIndexType, CeremonyPhaseType},
};
use frame_support::{
    assert_ok,
    traits::{OnFinalize, OnInitialize}
};
use rstest::*;
use sp_core::{sr25519, Pair, U256};
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
    let _ = pallet_timestamp::Pallet::<TestRuntime>::set(Origin::none(),t);
}

/// get correct meetup time for a certain cid and meetup
fn correct_meetup_time(cid: &CommunityIdentifier, mindex: MeetupLocationIndexType) -> Moment {
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
    mindex: MeetupLocationIndexType,
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
    mindex: MeetupLocationIndexType,
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
    let time = correct_meetup_time(&cid, 1);

    for i in 0..bootstrappers.len() {
        let mut bs = bootstrappers.clone();
        let claimant = bs.remove(i);
        attest_all(account_id(&claimant), &bs.iter().collect(), cid, cindex, 1, loc, time, 6);
    }
    run_to_next_phase();
    // REGISTERING
    cid
}

fn fully_attest_meetups(
    cid: CommunityIdentifier,
    keys: Vec<sr25519::Pair>,
) {
    get_meetups(cid).into_iter().for_each(|meetup|
        fully_attest_meetup(cid, keys.clone(), meetup));
}

/// perform full attestation of all participants for a given meetup
fn fully_attest_meetup(
    cid: CommunityIdentifier,
    keys: Vec<sr25519::Pair>,
    meetup: Vec<AccountId>,
) {
    for p in meetup.iter() {

        let others: Vec<&sr25519::Pair> = meetup.iter()
            .filter(|o| o != &p)
            .map(|o| keys.iter().filter(move |pair| account_id(pair) == *o))
            .flatten()
            .collect();

        let mindex = EncointerCeremonies::meetup_location_index((cid, EncointerScheduler::current_ceremony_index()), meetup.get(0).unwrap());
        let loc  = EncointerCommunities::get_locations(&cid)[(mindex - 1) as usize];
        let time = correct_meetup_time(&cid, mindex);
        attest_all(
            (*p).clone(),
            &others,
            cid,
            EncointerScheduler::current_ceremony_index(),
            mindex,
            loc,
            time,
            meetup.len() as u32,
        );
    }
}

fn assert_error(actual: DispatchResult, expected: Error::<TestRuntime>) {
    assert_eq!(match actual.clone().err().unwrap() {
        sp_runtime::DispatchError::Module { index: _, error: _, message } => message,
        _ => panic!(),
    }.unwrap(), expected.as_str());
}

fn get_meetups(cid: CommunityIdentifier) -> Vec<Vec<AccountId>> {
    MeetupRegistry::<TestRuntime>::iter_prefix_values((
        cid,
        EncointerScheduler::current_ceremony_index(),
    )).collect()
}

fn get_meetups_by_size(cid: CommunityIdentifier, size: usize) -> Vec<Vec<AccountId>> {
    get_meetups(cid).into_iter().filter(|meetup| meetup.len() == size).collect()
}

// unit tests ////////////////////////////////////////

#[test]
fn registering_participant_works() {
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        let bob = AccountId::from(AccountKeyring::Bob);
        let cindex = EncointerScheduler::current_ceremony_index();
        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 0);
        assert_ok!(register(alice.clone(), cid, None));

        assert_eq!(EncointerCeremonies::participant_count((cid, cindex)), 1);
        assert_ok!(register(bob.clone(), cid, None));

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
            EncointerCeremonies::meetup_location_index((cid, cindex), &account_id(&alice)),
            1
        );
        let loc = Location::default();
        let time = correct_meetup_time(&cid, 1);
        attest_all(
            account_id(&alice),
            &vec![&bob, &ferdie],
            cid,
            1,
            1,
            loc,
            time,
            3,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice, &ferdie],
            cid,
            1,
            1,
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
            1,
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
            1,
            Location::default(),
            correct_meetup_time(&cid, 1),
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
        let time = correct_meetup_time(&cid, 1);
        eve_claims.insert(
            0,
            signed_claim(
                &alice,
                cid,
                cindex,
                1,
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
                1,
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
            1,
            Location::default(),
            correct_meetup_time(&cid, 1),
            3,
        );
        assert_eq!(EncointerCeremonies::attestation_count((cid, cindex)), 1);
        let wit_vec = EncointerCeremonies::attestation_registry((cid, cindex), &1);
        assert!(wit_vec.contains(&account_id(&eve)) == false);
        assert!(wit_vec.len() == 1);
    });
}

#[test]
fn attest_claims_with_wrong_meetup_location_index_fails() {
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
        let time = correct_meetup_time(&cid, 1);
        let mut alice_claims: Vec<TestClaim> = vec![];
        alice_claims.push(
            signed_claim(&bob, cid, 1, 1, loc, time, 3),
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
        let time = correct_meetup_time(&cid, 1);
        let mut alice_attestations: Vec<TestClaim> = vec![];
        alice_attestations.push(
            signed_claim(&bob, cid, 1, 1, loc, time, 3),
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
        let time = correct_meetup_time(&cid, 1) + TIME_TOLERANCE + 1;
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
        let time = correct_meetup_time(&cid, 1);
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
        let time = correct_meetup_time(&cid, 1);
        attest_all(
            account_id(&alice),
            &vec![&bob, &charlie, &dave, &eve, &ferdie],
            cid,
            cindex,
            1,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&ferdie),
            &vec![&alice],
            cid,
            cindex,
            1,
            loc,
            time,
            6,
        );
        // assert that majority vote was successful
        assert_eq!(
            EncointerCeremonies::ballot_meetup_n_votes(&cid, cindex, 1),
            Some((5, 5))
        );

        attest_all(
            account_id(&alice),
            &vec![&bob],
            cid,
            1,
            1,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice],
            cid,
            1,
            1,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&alice),
            &vec![&charlie, &dave],
            cid,
            1,
            1,
            loc,
            time,
            4,
        );
        attest_all(
            account_id(&alice),
            &vec![&eve, &ferdie],
            cid,
            1,
            1,
            loc,
            time,
            6,
        );
        // votes should be (4, 2), (5, 2), (6, 2)
        assert!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 1) == None);

        attest_all(
            account_id(&alice),
            &vec![&bob, &charlie],
            cid,
            1,
            1,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&bob),
            &vec![&alice],
            cid,
            1,
            1,
            loc,
            time,
            5,
        );
        attest_all(
            account_id(&alice),
            &vec![&dave],
            cid,
            1,
            1,
            loc,
            time,
            4,
        );
        attest_all(
            account_id(&alice),
            &vec![&eve, &ferdie],
            cid,
            1,
            1,
            loc,
            time,
            6,
        );
        // votes should be (5, 3), (6, 2), (4, 1)
        assert_eq!(EncointerCeremonies::ballot_meetup_n_votes(&cid, 1, 1), Some((5, 3)));
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
        let time = correct_meetup_time(&cid, 1);

        let claim_base = TestClaim::new_unsigned(
            account_id(&alice),
            cindex,
            cid,
            1,
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
            (EncointerCeremonies::get_meetup_time(&cid, 1).unwrap() as i64 -
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
    new_test_ext().execute_with(|| {
        let cid = register_test_community::<TestRuntime>(None, 0.0, 0.0);
        let alice = AccountId::from(AccountKeyring::Alice);
        let cindex = EncointerScheduler::current_ceremony_index();
        register_alice_bob_ferdie(cid);
        assert_eq!(
            EncointerCeremonies::meetup_location_index((cid, cindex), &alice),
            NONE
        );
        run_to_next_phase();
        assert_eq!(EncointerCeremonies::meetup_location_index((cid, cindex), &alice), 1);
        run_to_next_phase();
        run_to_next_phase();
        assert_eq!(
            EncointerCeremonies::meetup_location_index((cid, cindex), &alice),
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

        run_to_next_phase();

        // ASSIGNING
        assert_eq!(get_meetups(cid).len(), 1);
        // whitepaper III-B Rule 3: no more than 1/3 participants without reputation
        assert_eq!(get_meetups_by_size(cid, 9).len(), 1);

        run_to_next_phase();
        // WITNESSING

        fully_attest_meetups(cid, participants.clone());

        run_to_next_phase();
        // REGISTERING

        // register everybody again. also those who didn't have the chance last time
        for pair in participants.iter() {
            let proof = get_proof(cid, EncointerScheduler::current_ceremony_index() - 1, pair);
            register(account_id(&pair), cid, proof).unwrap();
        }
        run_to_next_phase();

        // ASSIGNING
        let meetups: Vec<Vec<AccountId>> = get_meetups(cid);
        assert_eq!(meetups.len(), 2);
        // whitepaper III-B Rule 3: no more than 1/3 participants without reputation
        assert_eq!(get_meetups_by_size(cid, 7).len(), 1);
        assert_eq!(get_meetups_by_size(cid, 6).len(), 1);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetups(cid, participants.clone());

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

        let meetups: Vec<Vec<AccountId>> = get_meetups(cid);
        assert_eq!(meetups.len(), 2);
        // whitepaper III-B Rule 3: no more than 1/3 participants without reputation
        assert_eq!(get_meetups_by_size(cid, 10).len(), 1);
        assert_eq!(get_meetups_by_size(cid, 9).len(), 1);

        run_to_next_phase();
        // WITNESSING
        fully_attest_meetups(cid, participants.clone());

        run_to_next_phase();
        // REGISTERING

        // TODO: whitepaper III-B Rule 1: minimize the number of participants that have met at previous ceremony
        // TODO: whitepaper III-B Rule 2: maximize number of participants per meetup within 3<=N<=12
    });
}

#[rstest(n_bootstrappers, n_reputables, n_endorsees, n_newbies, n_locations, exp_meetups,
    case(8,0,0,4,3, vec![12]),
    case(9,0,0,4,3, vec![7,6]),
    case(3,7,3,3,3, vec![8,8]),
    case(3,7,4,3,3, vec![9,8]),
    case::do_not_assign_more_meetups_than_locations(3,7,50,0,3, vec![12,12,12]),
    case::do_not_assign_more_meetups_than_there_are_experienced_participants(3,1,49,0,10, vec![12,12,12,12]),
    case(12,48,12*AMOUNT_NEWBIE_TICKETS as usize,0,55, [12; 55].to_vec()),
)]
fn assigning_meetup_works(
    n_bootstrappers: usize,
    n_reputables: usize,
    n_endorsees: usize,
    n_newbies: usize,
    n_locations: u32,
    exp_meetups: Vec<usize>,
) {
    new_test_ext().execute_with(|| {
        let bs = add_population(n_bootstrappers, 0);
        let cid = perform_bootstrapping_ceremony(Some(bs.clone()), n_locations);
        let mut population: Vec<sr25519::Pair> = bs.clone();

        if n_reputables > 0 {
            // setup the community to be able to test assignment with given parameters
            population = grow_community(population, cid, n_bootstrappers + n_reputables);
            assert_eq!(population.len(), n_bootstrappers + n_reputables);
        }

        for e in 0..n_endorsees {
            population.extend(add_population(1, population.len()));
            assert_ok!(EncointerCeremonies::endorse_newcomer(
                Origin::signed(account_id(&bs[e % n_bootstrappers]).clone()),
                cid,
                account_id(&population.last().unwrap())
            ));
        }

        population.extend(add_population(n_newbies, population.len()));
        assert_eq!(
            population.len(),
            n_bootstrappers + n_reputables + n_endorsees + n_newbies
        );

        // setup finished. Now registering all participants

        let cindex = EncointerScheduler::current_ceremony_index();
        population
            .iter()
            .for_each(|p| register(account_id(&p), cid, get_proof(cid, cindex - 1, p)).unwrap());

        run_to_next_phase(); // ASSIGNING

        assert_eq!(
            EncointerCeremonies::meetup_count((cid, cindex)),
            exp_meetups.len() as u64
        );

        for size in 0..exp_meetups.iter().map(|v| *v as MeetupLocationIndexType).max().unwrap(){
            assert_eq!(
                get_meetups_by_size(cid, size as usize).len(),
                exp_meetups.iter().filter(|v| *v == &(size as usize)).count()
            )
        }
    });
}

/// Grows the community until the specified amount. Returns all the key pairs of the community.
fn grow_community(
    bootstrappers: Vec<sr25519::Pair>,
    cid: CommunityIdentifier,
    amount: usize,
) -> Vec<sr25519::Pair> {
    assert!(bootstrappers.len() < amount as usize);

    let mut participants = bootstrappers;
    let curr_pop_size = participants.len();
    participants.extend(add_population(amount - curr_pop_size, curr_pop_size));

    let cindex = EncointerScheduler::current_ceremony_index();

    let mut proofs: Vec<Option<TestProofOfAttendance>> = participants
        .iter()
        .map(|p| get_proof(cid, cindex - 1, p))
        .collect();

    // the amount of proofs we get is the current amount bootstrappers + reputables (== whole community)
    // if we assume that everyone participated in the last meetup.
    while proofs.clone().iter().filter(|p| p.is_some()).count() < amount {
        for (i, p) in participants.iter().enumerate() {
            register(account_id(&p), cid, proofs[i].clone()).unwrap();
        }

        let cindex = EncointerScheduler::current_ceremony_index();
        run_to_next_phase(); // ASSIGNING

        let m_count = EncointerCeremonies::meetup_count((cid, cindex));
        assert!(m_count > 0);
        run_to_next_phase(); // WITNESSING

        fully_attest_meetups(cid, participants.clone());

        run_to_next_phase(); // REGISTERING

        let cindex = EncointerScheduler::current_ceremony_index();
        proofs = participants
            .iter()
            .map(|p| get_proof(cid, cindex - 1, p))
            .collect();

        // sanity check that everything worked
        assert!(proofs.clone().iter().filter(|p| p.is_some()).count() > 0);
    }

    participants
}
