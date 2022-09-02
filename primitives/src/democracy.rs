use crate::{
	ceremonies::{CommunityCeremony, InactivityTimeoutType},
	communities::{CommunityIdentifier, NominalIncome as NominalIncomeType},
};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[cfg(feature = "serde_derive")]
use serde::{Deserialize, Serialize};
use sp_core::RuntimeDebug;
use sp_runtime::BoundedVec;

pub type ProposalIdType = u128;
pub type VoteCountType = u128;
pub type VoteEntry<AccountId> = (AccountId, CommunityCeremony);
pub type ReputationVec<MaxLength> = BoundedVec<CommunityCeremony, MaxLength>;

#[derive(
	Encode, Decode, Default, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen,
)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Tally {
	pub turnout: VoteCountType,
	pub ayes: VoteCountType,
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum Vote {
	Aye,
	Nay,
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalAction {
	Global(GlobalProposalAction),
	Community(CommunityIdentifier, CommunityProposalAction),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum CommunityProposalAction {
	UpdateNominalIncome(NominalIncomeType),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum GlobalProposalAction {
	SetInactivityTimeout(InactivityTimeoutType),
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
}
