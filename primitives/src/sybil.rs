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
        sybil_proof_issuer_index: u8,
        requester: AccountId,
        request: ProofOfPersonhoodRequest<Signature, AccountId>,
        sender_pallet_index: u8,
    ) -> Self {
        Self {
            call_index: [sybil_proof_issuer_index, 0], // is the first call in proof-issuer pallet
            requester,
            request,
            sender_pallet_index,
        }
    }
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct SetProofOfPersonHoodCall<AccountId> {
    call_index: [u8; 2],
    account: AccountId,
    confidence: ProofOfPersonhoodConfidence,
}

impl<AccountId> SetProofOfPersonHoodCall<AccountId> {
    pub fn new(
        sybil_gate_index: u8,
        account: AccountId,
        confidence: ProofOfPersonhoodConfidence,
    ) -> Self {
        Self {
            call_index: [sybil_gate_index, 1],
            account,
            confidence,
        }
    }
}

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
