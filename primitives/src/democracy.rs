use crate::{
	ceremonies::{CommunityCeremony, InactivityTimeoutType},
	communities::{CommunityIdentifier, NominalIncome as NominalIncomeType},
};
use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::scheduler::CeremonyIndexType;
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
pub enum ProposalAccessPolicy {
	Global,
	Community(CommunityIdentifier),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalAction {
	UpdateNominalIncome(CommunityIdentifier, NominalIncomeType),
	SetInactivityTimeout(InactivityTimeoutType),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalActionIdentifier {
	UpdateNominalIncome(CommunityIdentifier),
	SetInactivityTimeout,
}

impl ProposalAction {
	pub fn get_access_policy(self) -> ProposalAccessPolicy {
		match self {
			ProposalAction::UpdateNominalIncome(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::SetInactivityTimeout(_) => ProposalAccessPolicy::Global,
		}
	}

	pub fn get_identifier(self) -> ProposalActionIdentifier {
		match self {
			ProposalAction::UpdateNominalIncome(cid, _) =>
				ProposalActionIdentifier::UpdateNominalIncome(cid),
			ProposalAction::SetInactivityTimeout(_) =>
				ProposalActionIdentifier::SetInactivityTimeout,
		}
	}
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalState<BlockNumber> {
	Ongoing,
	Confirming { since: BlockNumber },
	Approved,
	Cancelled,
	Enacted,
}

impl<BlockNumber: std::cmp::PartialEq> ProposalState<BlockNumber> {
	pub fn can_update(self) -> bool {
		matches!(self, Self::Confirming { since: _ } | Self::Ongoing)
	}
}
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Proposal<BlockNumber> {
	pub start: BlockNumber,
	pub start_cindex: CeremonyIndexType,
	pub action: ProposalAction,
	pub state: ProposalState<BlockNumber>,
}
