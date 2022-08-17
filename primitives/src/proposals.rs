use crate::communities::{CommunityIdentifier, Location, NominalIncome as NominalIncomeType};
use codec::{Decode, Encode, MaxEncodedLen};
use ep_core::fixed::types::I64F64;
use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Convert;

use crate::fixed::{
	traits::ToFixed,
	types::{U64F64, U66F62},
};

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalAction {
	UpdateNominalIncome(CommunityIdentifier, NominalIncomeType),
}
#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalAccessPolicy {
	Global,
	Community(CommunityIdentifier),
}
#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalState<BlockNumber> {
	Ongoing,
	Confirming { since: BlockNumber },
	Approved { since: BlockNumber },
	Cancelled,
}
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Proposal<BlockNumber> {
	start: BlockNumber,
	proposal_type: ProposalAction,
	state: ProposalState<BlockNumber>,
	access_policy: ProposalAccessPolicy,
}
