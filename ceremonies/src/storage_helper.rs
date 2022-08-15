use codec::{Decode, EncodeLike};
use encointer_primitives::{
	ceremonies::{CommunityCeremony, ParticipantIndexType},
	communities::CommunityIdentifier,
	scheduler::CeremonyIndexType,
};
pub fn remove_participant_from_registry<Index, Registry, Count, AccountId>(
	cid: CommunityIdentifier,
	cindex: CeremonyIndexType,
	participant: &AccountId,
) where
	Index: frame_support::StorageDoubleMap<
		CommunityCeremony,
		AccountId,
		ParticipantIndexType,
		Query = ParticipantIndexType,
	>,
	Registry: frame_support::StorageDoubleMap<
		CommunityCeremony,
		ParticipantIndexType,
		AccountId,
		Query = Option<AccountId>,
	>,
	Count: frame_support::StorageMap<
		CommunityCeremony,
		ParticipantIndexType,
		Query = ParticipantIndexType,
	>,
	AccountId: EncodeLike + Decode,
{
	let participant_count = Count::get((cid, cindex));
	let participant_index = Index::get((cid, cindex), &participant);
	let maybe_last_participant = Registry::get((cid, cindex), participant_count);

	if let Some(last_participant) = maybe_last_participant {
		Registry::insert((cid, cindex), participant_index, &last_participant);
		Index::insert((cid, cindex), last_participant, participant_index);

		Registry::remove((cid, cindex), participant_count);
		Index::remove((cid, cindex), &participant);

		Count::insert((cid, cindex), participant_count.saturating_sub(1));
	}
}

