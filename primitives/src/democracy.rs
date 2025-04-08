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

use crate::{
	ceremonies::ReputationCountType,
	common::PalletString,
	scheduler::CeremonyIndexType,
	treasuries::{SwapAssetOption, SwapNativeOption},
};
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
pub enum ProposalAction<AccountId, Balance, Moment, AssetId> {
	AddLocation(CommunityIdentifier, Location),
	RemoveLocation(CommunityIdentifier, Location),
	UpdateCommunityMetadata(CommunityIdentifier, CommunityMetadataType),
	UpdateDemurrage(CommunityIdentifier, Demurrage),
	UpdateNominalIncome(CommunityIdentifier, NominalIncomeType),
	SetInactivityTimeout(InactivityTimeoutType),
	Petition(Option<CommunityIdentifier>, PalletString),
	SpendNative(Option<CommunityIdentifier>, AccountId, Balance),
	IssueSwapNativeOption(CommunityIdentifier, AccountId, SwapNativeOption<Balance, Moment>),
	SpendAsset(Option<CommunityIdentifier>, AccountId, Balance, AssetId),
	IssueSwapAssetOption(CommunityIdentifier, AccountId, SwapAssetOption<Balance, Moment, AssetId>),
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
	Petition(Option<CommunityIdentifier>),
	SpendNative(Option<CommunityIdentifier>),
	IssueSwapNativeOption(CommunityIdentifier),
	SpendAsset(Option<CommunityIdentifier>),
	IssueSwapAssetOption(CommunityIdentifier),
}

impl<AccountId, Balance, Moment, AssetId> ProposalAction<AccountId, Balance, Moment, AssetId> {
	pub fn get_access_policy(&self) -> ProposalAccessPolicy {
		match self {
			ProposalAction::AddLocation(cid, _) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::RemoveLocation(cid, _) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::UpdateCommunityMetadata(cid, _) =>
				ProposalAccessPolicy::Community(*cid),
			ProposalAction::UpdateDemurrage(cid, _) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::UpdateNominalIncome(cid, _) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::SetInactivityTimeout(_) => ProposalAccessPolicy::Global,
			ProposalAction::Petition(Some(cid), _) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::Petition(None, _) => ProposalAccessPolicy::Global,
			ProposalAction::SpendNative(Some(cid), ..) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::SpendNative(None, ..) => ProposalAccessPolicy::Global,
			ProposalAction::IssueSwapNativeOption(cid, ..) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::SpendAsset(Some(cid), ..) => ProposalAccessPolicy::Community(*cid),
			ProposalAction::SpendAsset(None, ..) => ProposalAccessPolicy::Global,
			ProposalAction::IssueSwapAssetOption(cid, ..) => ProposalAccessPolicy::Community(*cid),
		}
	}

	pub fn get_identifier(&self) -> ProposalActionIdentifier {
		match self {
			ProposalAction::AddLocation(cid, _) => ProposalActionIdentifier::AddLocation(*cid),
			ProposalAction::RemoveLocation(cid, _) =>
				ProposalActionIdentifier::RemoveLocation(*cid),
			ProposalAction::UpdateCommunityMetadata(cid, _) =>
				ProposalActionIdentifier::UpdateCommunityMetadata(*cid),
			ProposalAction::UpdateDemurrage(cid, _) =>
				ProposalActionIdentifier::UpdateDemurrage(*cid),
			ProposalAction::UpdateNominalIncome(cid, _) =>
				ProposalActionIdentifier::UpdateNominalIncome(*cid),
			ProposalAction::SetInactivityTimeout(_) =>
				ProposalActionIdentifier::SetInactivityTimeout,
			ProposalAction::Petition(maybe_cid, _) =>
				ProposalActionIdentifier::Petition(*maybe_cid),
			ProposalAction::SpendNative(maybe_cid, ..) =>
				ProposalActionIdentifier::SpendNative(*maybe_cid),
			ProposalAction::IssueSwapNativeOption(cid, ..) =>
				ProposalActionIdentifier::IssueSwapNativeOption(*cid),
			ProposalAction::SpendAsset(maybe_cid, ..) =>
				ProposalActionIdentifier::SpendNative(*maybe_cid),
			ProposalAction::IssueSwapAssetOption(cid, ..) =>
				ProposalActionIdentifier::IssueSwapNativeOption(*cid),
		}
	}

	/// Returns true if the action supersedes other proposals of the same action type when approved.
	pub fn supersedes_same_action(&self) -> bool {
		match self {
			ProposalAction::AddLocation(_, _) => true,
			ProposalAction::RemoveLocation(_, _) => true,
			ProposalAction::UpdateCommunityMetadata(_, _) => true,
			ProposalAction::UpdateDemurrage(_, _) => true,
			ProposalAction::UpdateNominalIncome(_, _) => true,
			ProposalAction::SetInactivityTimeout(_) => true,
			ProposalAction::Petition(_, _) => false,
			ProposalAction::SpendNative(_, _, _) => false,
			ProposalAction::IssueSwapNativeOption(..) => false,
			ProposalAction::SpendAsset(_, _, _, _) => false,
			ProposalAction::IssueSwapAssetOption(..) => false,
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
pub struct Proposal<Moment, AccountId, Balance, AssetId> {
	pub start: Moment,
	pub start_cindex: CeremonyIndexType,
	pub action: ProposalAction<AccountId, Balance, Moment, AssetId>,
	pub state: ProposalState<Moment>,
	pub electorate_size: ReputationCountType,
}
