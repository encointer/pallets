use codec::{Decode, Encode};
use fixed::traits::Fixed;
use rstd::vec::Vec;
use sp_core::RuntimeDebug;

use crate::ceremonies::ProofOfAttendance;
use crate::scheduler::CeremonyIndexType;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct IssueProofOfPersonhoodConfidenceCall<Signature, AccountId> {
    call_index: [u8; 2],
    requester: AccountId,
    request: Vec<ProofOfAttendance<Signature, AccountId>>,
    requested_response: u8,
    sender_pallet_index: u8,
}

impl<Signature, AccountId> IssueProofOfPersonhoodConfidenceCall<Signature, AccountId> {
    pub fn new(
        sybil_proof_issuer_index: u8,
        requester: AccountId,
        request: Vec<ProofOfAttendance<Signature, AccountId>>,
        requested_response: RequestedSybilResponse,
        sender_pallet_index: u8,
    ) -> Self {
        Self {
            call_index: [sybil_proof_issuer_index, 0], // is the first call in proof-issuer pallet
            requester,
            request,
            requested_response: requested_response as u8,
            sender_pallet_index,
        }
    }
}

/// This allows to generically call the sybil-proof-issuer, whose response calls the method with the
/// index defined in the `RequestedSybilResponse`
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum RequestedSybilResponse {
    Faucet = 1,
}

impl Default for RequestedSybilResponse {
    fn default() -> RequestedSybilResponse {
        RequestedSybilResponse::Faucet
    }
}

#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct SybilResponseCall<AccountId> {
    call_index: [u8; 2],
    account: AccountId,
    confidence: ProofOfPersonhoodConfidence,
}

impl<AccountId> SybilResponseCall<AccountId> {
    pub fn new(
        sybil_gate_index: u8,
        requested_sybil_response_call_index: u8,
        account: AccountId,
        confidence: ProofOfPersonhoodConfidence,
    ) -> Self {
        Self {
            call_index: [sybil_gate_index, requested_sybil_response_call_index],
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

    pub fn as_ratio<F: Fixed>(&self) -> F {
        return F::from_num(self.attested)
            .checked_div(F::from_num(self.last_n_ceremonies))
            .unwrap_or_default();
    }
}
