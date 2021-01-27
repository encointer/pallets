use codec::{Decode, Encode};
use sp_core::RuntimeDebug;

use crate::ceremonies::ProofOfAttendance;
use crate::communities::CommunityIdentifier;

pub type ProofOfPersonhoodRequest<Signature, AccountId> =
    Vec<(CommunityIdentifier, ProofOfAttendance<Signature, AccountId>)>;

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfPersonhoodConfidence {
    attested: u8,
    last_n_ceremonies: u8,
}
