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
use crate::mock::{Origin, new_test_ext, TestRuntime};
use codec::Decode;
use frame_support::assert_ok;

use xcm_executor::traits::Convert;

use encointer_primitives::sybil::consts::SYBIL_CALL_WEIGHT;
use test_utils::{storage::*, *};

pub type PersonhoodOracle = crate::Module<TestRuntime>;

/// ProofOfAttendance generated by encointer-client for the community of the bootstrap demo script
fn proof_of_attendance() -> ProofOfAttendance<Signature, AccountId> {
    let proof_hex = "d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d020000002cbd65a5f087b3d60aec997e6369ef694f125582f5f7cffd7bbddc56a71858fcd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d0172b56983d1fd9d53043ffd1406427da13a350b0d81e89a81c3061e9bca09825a4fdfb0a8d5baf0fb901a4e0155195703f3d60c51b5e1a8be16b166779798e789";

    hex::decode(proof_hex)
        .map(|p| ProofOfAttendance::decode(&mut p.as_slice()))
        .unwrap()
        .unwrap()
}

#[test]
fn issue_proof_of_personhood_is_ok() {
    new_test_ext().execute_with(|| {
        let sibling = sibling_junction(1863);
        let account_id = LocationConverter::convert_ref(&sibling.into()).unwrap();
        assert_ok!(PersonhoodOracle::issue_personhood_uniqueness_rating(
            Origin::signed(account_id),
            vec![proof_of_attendance()].encode(),
            CallMetadata::new(1, 1, SYBIL_CALL_WEIGHT),
        ));
    })
}

#[test]
fn create_proof_of_personhood_confidence_works() {
    let proof = proof_of_attendance();
    // println!("ProofOfAttendance: {:?}", proof);
    let acc = proof.attendee_public.clone();
    let cid = proof.community_identifier;
    let cindex = proof.ceremony_index;

    let mut ext = new_test_ext();
    ext.insert(current_ceremony_index(), 3.encode());
    ext.insert(community_identifiers(), vec![cid].encode());
    ext.insert(
        participant_reputation((cid, cindex), acc),
        Reputation::VerifiedUnlinked.encode(),
    );

    ext.execute_with(|| {
        assert_eq!(
            PersonhoodOracle::verify(vec![proof.clone()]).unwrap(),
            PersonhoodUniquenessRating::new(1, 1, vec![proof.hash()])
        )
    })
}

#[test]
fn account_id_conversion_works() {
    new_test_ext().execute_with(|| {
        let sibling = sibling_junction(1863);
        let account = LocationConverter::convert_ref(&sibling.clone().into()).unwrap();
        assert_eq!(
            LocationConverter::reverse_ref(account).unwrap(),
            sibling.into()
        );
    });
}
