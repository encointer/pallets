use crate::{
	balances::Demurrage,
	ceremonies::{CommunityCeremony, InactivityTimeoutType},
	communities::{
		CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Location,
		NominalIncome as NominalIncomeType,
	},
};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

use crate::{ceremonies::ReputationCountType, scheduler::CeremonyIndexType};
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

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalAction {
	AddLocation(CommunityIdentifier, Location),
	RemoveLocation(CommunityIdentifier, Location),
	UpdateCommunityMetadata(CommunityIdentifier, CommunityMetadataType),
	UpdateDemurrage(CommunityIdentifier, Demurrage),
	UpdateNominalIncome(CommunityIdentifier, NominalIncomeType),
	SetInactivityTimeout(InactivityTimeoutType),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, Copy, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub enum ProposalActionIdentifier {
	AddLocation(CommunityIdentifier),
	RemoveLocation(CommunityIdentifier),
	UpdateCommunityMetadata(CommunityIdentifier),
	UpdateDemurrage(CommunityIdentifier),
	UpdateNominalIncome(CommunityIdentifier),
	SetInactivityTimeout,
}

impl ProposalAction {
	pub fn get_access_policy(self) -> ProposalAccessPolicy {
		match self {
			ProposalAction::AddLocation(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::RemoveLocation(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::UpdateCommunityMetadata(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::UpdateDemurrage(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::UpdateNominalIncome(cid, _) => ProposalAccessPolicy::Community(cid),
			ProposalAction::SetInactivityTimeout(_) => ProposalAccessPolicy::Global,
		}
	}

	pub fn get_identifier(self) -> ProposalActionIdentifier {
		match self {
			ProposalAction::AddLocation(cid, _) => ProposalActionIdentifier::AddLocation(cid),
			ProposalAction::RemoveLocation(cid, _) => ProposalActionIdentifier::RemoveLocation(cid),
			ProposalAction::UpdateCommunityMetadata(cid, _) =>
				ProposalActionIdentifier::UpdateCommunityMetadata(cid),
			ProposalAction::UpdateDemurrage(cid, _) =>
				ProposalActionIdentifier::UpdateDemurrage(cid),
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
pub enum ProposalState<Moment> {
	Ongoing,
	Confirming { since: Moment },
	Approved,
	SupersededBy { id: ProposalIdType },
	Rejected,
	Enacted,
}

impl<Moment: PartialEq> ProposalState<Moment> {
	pub fn can_update(self) -> bool {
		matches!(self, Self::Confirming { since: _ } | Self::Ongoing)
	}

	pub fn has_failed(self) -> bool {
		matches!(self, Self::SupersededBy { id: _ } | Self::Rejected)
	}
}
#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "serde_derive", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde_derive", serde(rename_all = "camelCase"))]
pub struct Proposal<Moment> {
	pub start: Moment,
	pub start_cindex: CeremonyIndexType,
	pub action: ProposalAction,
	pub state: ProposalState<Moment>,
	pub electorate_size: ReputationCountType,
}
