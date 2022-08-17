use crate::communities::{CommunityIdentifier, NominalIncome as NominalIncomeType};
use codec::{Decode, Encode, MaxEncodedLen};

use scale_info::TypeInfo;
use sp_core::RuntimeDebug;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};




pub type ProposalIdType = u128;

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
	pub start: BlockNumber,
	pub action: ProposalAction,
	pub state: ProposalState<BlockNumber>,
	pub access_policy: ProposalAccessPolicy,
}
