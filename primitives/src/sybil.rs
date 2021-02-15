use codec::{Decode, Encode};
use fixed::traits::Fixed;
use rstd::vec::Vec;
use sp_core::{RuntimeDebug, H256};
use sp_runtime::traits::{BlakeTwo256, Hash};

use crate::scheduler::CeremonyIndexType;

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct IssueProofOfPersonhoodConfidenceCall {
    call_index: [u8; 2],
    request: ProofOfPersonhoodRequest,
    requested_response: u8,
    sender_pallet_index: u8,
}

pub type ProofOfPersonhoodRequest = Vec<u8>;

pub trait RequestHash {
    fn hash(&self) -> H256;
}

impl RequestHash for ProofOfPersonhoodRequest {
    fn hash(&self) -> H256 {
        self.using_encoded(BlakeTwo256::hash)
    }
}

impl IssueProofOfPersonhoodConfidenceCall {
    pub fn new(
        sybil_proof_issuer_index: u8,
        request: Vec<u8>,
        requested_response: RequestedSybilResponse,
        sender_pallet_index: u8,
    ) -> Self {
        Self {
            call_index: [sybil_proof_issuer_index, 0], // is the first call in personhood-oracle pallet
            request,
            requested_response: requested_response as u8,
            sender_pallet_index,
        }
    }

    pub fn request_hash(&self) -> H256 {
        self.request.hash()
    }
}

/// This allows to generically call the sybil-personhood-oracle, whose response calls the method with the
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

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct SybilResponseCall {
    call_index: [u8; 2],
    request_hash: H256,
    confidence: ProofOfPersonhoodConfidence,
}

impl SybilResponseCall {
    pub fn new(
        sybil_gate_index: u8,
        requested_sybil_response_call_index: u8,
        request_hash: H256,
        confidence: ProofOfPersonhoodConfidence,
    ) -> Self {
        Self {
            call_index: [sybil_gate_index, requested_sybil_response_call_index],
            request_hash,
            confidence,
        }
    }
}

#[derive(Encode, Decode, Clone, PartialEq, Eq, Default, RuntimeDebug)]
pub struct ProofOfPersonhoodConfidence {
    attested: CeremonyIndexType,
    last_n_ceremonies: CeremonyIndexType,
    proofs: Vec<H256>,
}

impl ProofOfPersonhoodConfidence {
    pub fn new(
        attested: CeremonyIndexType,
        last_n_ceremonies: CeremonyIndexType,
        proofs: Vec<H256>,
    ) -> Self {
        Self {
            attested,
            last_n_ceremonies,
            proofs,
        }
    }

    pub fn proofs(&self) -> Vec<H256> {
        self.proofs.clone()
    }

    pub fn as_ratio<F: Fixed>(&self) -> F {
        return F::from_num(self.attested)
            .checked_div(F::from_num(self.last_n_ceremonies))
            .unwrap_or_default();
    }
}
