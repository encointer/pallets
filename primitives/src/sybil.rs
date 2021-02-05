use codec::{Decode, Encode};
use rstd::vec::Vec;
use sp_core::RuntimeDebug;

use crate::ceremonies::ProofOfAttendance;
use crate::communities::CommunityIdentifier;
use crate::scheduler::CeremonyIndexType;

pub type ProofOfPersonhoodRequest<Signature, AccountId> =
    Vec<(CommunityIdentifier, ProofOfAttendance<Signature, AccountId>)>;

pub type IssueProofOfPersonhoodConfidenceCall<Signature, AccountId> = (
    [u8; 2],
    u32,
    u8,
    ProofOfPersonhoodRequest<Signature, AccountId>,
);

pub type SetProofOfPersonHoodCall<AccountId> = ([u8; 2], AccountId, ProofOfPersonhoodConfidence);

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfPersonhoodConfidence {
    attested: CeremonyIndexType,
    last_n_ceremonies: CeremonyIndexType,
}

impl ProofOfPersonhoodConfidence {
    pub fn new(attested: CeremonyIndexType, last_n_ceremonies: CeremonyIndexType) -> Self {
        Self {
            attested,
            last_n_ceremonies,
        }
    }
}
