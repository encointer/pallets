use codec::{Decode, Encode};
use rstd::vec::Vec;
use sp_core::RuntimeDebug;

use crate::ceremonies::ProofOfAttendance;
use crate::communities::CommunityIdentifier;
use crate::scheduler::CeremonyIndexType;

pub type ProofOfPersonhoodRequest<Signature, AccountId> =
    Vec<(CommunityIdentifier, ProofOfAttendance<Signature, AccountId>)>;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct IssueProofOfPersonhoodConfidenceCall<Signature, AccountId> {
    call_index: [u8; 2],
    requester: AccountId,
    request: ProofOfPersonhoodRequest<Signature, AccountId>,
    sender_pallet_index: u8,
}

impl<Signature, AccountId> IssueProofOfPersonhoodConfidenceCall<Signature, AccountId> {
    pub fn new(
        call_index: [u8; 2],
        requester: AccountId,
        request: ProofOfPersonhoodRequest<Signature, AccountId>,
        sender_pallet_index: u8,
    ) -> Self {
        Self {
            call_index,
            requester,
            request,
            sender_pallet_index,
        }
    }
}

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
