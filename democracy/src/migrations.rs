use super::*;

use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};

/// The log target.
const TARGET: &str = "democracy::migration::v1";

mod v0 {
	use super::*;
	use encointer_primitives::democracy::ProposalActionIdentifier;

	#[storage_alias]
	pub(super) type CancelledAt<T: Config> =
		StorageMap<Pallet<T>, Blake2_128Concat, ProposalActionIdentifier, u64, OptionQuery>;
}

pub mod v1 {
	use super::*;

	#[allow(dead_code)]
	pub struct MigrateV0toV1purging<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for MigrateV0toV1purging<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
			Ok(0u8.encode())
		}

		fn on_runtime_upgrade() -> Weight {
			let current_version = Pallet::<T>::in_code_storage_version();
			let onchain_version = Pallet::<T>::on_chain_storage_version();

			log::info!(
				target: TARGET,
				"Running migration with current storage version {current_version:?} / onchain {onchain_version:?}",
			);

			if onchain_version >= 1 {
				log::warn!(
					target: TARGET,
					"skipping on_runtime_upgrade: executed on wrong storage version."
				);
				return T::DbWeight::get().reads(1);
			}

			let mut purged_keys = 0u64;
			// this has been refactored to LastApprovedProposalForAction
			purged_keys += v0::CancelledAt::<T>::clear(u32::MAX, None).unique as u64;
			// ProposalState incompatible with new proposal struct
			purged_keys += Proposals::<T>::clear(u32::MAX, None).unique as u64;
			// ProposalState incompatible with new proposal struct
			purged_keys += EnactmentQueue::<T>::clear(u32::MAX, None).unique as u64;
			// we must keep ProposalCount and PurposeIds as we dont want to purge burnt rep

			StorageVersion::new(1).put::<Pallet<T>>();
			T::DbWeight::get().reads_writes(purged_keys, purged_keys + 1)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			assert_eq!(Pallet::<T>::on_chain_storage_version(), 1, "must upgrade");
			Ok(())
		}
	}
}

/// Old v1 storage format for EnactmentQueue (single ProposalIdType per key).
pub mod v1_storage {
	use super::*;
	use encointer_primitives::democracy::{ProposalActionIdentifier, ProposalIdType};

	#[storage_alias]
	pub type EnactmentQueue<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		ProposalActionIdentifier,
		ProposalIdType,
		OptionQuery,
	>;
}

pub mod v2 {
	use super::*;
	use encointer_primitives::democracy::{
		ProposalActionIdentifier, ProposalIdType, ProposalState,
	};

	pub struct MigrateV1toV2<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for MigrateV1toV2<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
			let count = v1_storage::EnactmentQueue::<T>::iter().count() as u32;
			log::info!(
				target: "democracy::migration::v2",
				"pre_upgrade: {count} entries in old EnactmentQueue"
			);
			Ok(count.encode())
		}

		fn on_runtime_upgrade() -> Weight {
			let onchain_version = Pallet::<T>::on_chain_storage_version();

			log::info!(
				target: "democracy::migration::v2",
				"Running migration with onchain storage version {onchain_version:?}",
			);

			if onchain_version != 1 {
				log::warn!(
					target: "democracy::migration::v2",
					"skipping migration: expected onchain version 1, got {onchain_version:?}"
				);
				return T::DbWeight::get().reads(1);
			}

			let mut reads = 1u64; // version read
			let mut writes = 0u64;

			// Step 1: Migrate existing v1 EnactmentQueue entries (single ProposalIdType)
			// to v2 format (BoundedVec<ProposalIdType>).
			let old_entries: Vec<(ProposalActionIdentifier, ProposalIdType)> =
				v1_storage::EnactmentQueue::<T>::iter().collect();
			reads += old_entries.len() as u64;

			// Clear the old storage completely
			let cleared = v1_storage::EnactmentQueue::<T>::clear(u32::MAX, None);
			writes += cleared.unique as u64;

			// Write entries in the new format
			for (action_id, proposal_id) in &old_entries {
				let mut queue = BoundedVec::default();
				if queue.try_push(*proposal_id).is_ok() {
					EnactmentQueue::<T>::insert(action_id, queue);
					writes += 1;
				}
			}

			// Step 2: Scan all proposals for Approved state not already in the queue.
			// These are proposals that were approved but lost due to the overwrite bug.
			let queued_ids: sp_std::collections::btree_set::BTreeSet<ProposalIdType> =
				old_entries.iter().map(|(_, id)| *id).collect();

			for (proposal_id, proposal) in Proposals::<T>::iter() {
				reads += 1;
				if proposal.state == ProposalState::Approved && !queued_ids.contains(&proposal_id) {
					let action_id = proposal.action.get_identifier();
					EnactmentQueue::<T>::mutate(action_id, |maybe_queue| {
						let queue = maybe_queue.get_or_insert_with(BoundedVec::default);
						if queue.try_push(proposal_id).is_err() {
							log::error!(
								target: "democracy::migration::v2",
								"EnactmentQueue overflow re-inserting approved proposal {proposal_id}"
							);
						}
					});
					writes += 1;
					log::info!(
						target: "democracy::migration::v2",
						"Re-inserted approved proposal {proposal_id} into enactment queue"
					);
				}
			}

			StorageVersion::new(2).put::<Pallet<T>>();
			writes += 1;

			log::info!(
				target: "democracy::migration::v2",
				"Migration complete: {reads} reads, {writes} writes"
			);
			T::DbWeight::get().reads_writes(reads, writes)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			assert_eq!(Pallet::<T>::on_chain_storage_version(), 2, "must upgrade to v2");

			// Verify no Approved proposals are missing from the queue
			let queued_ids: sp_std::collections::btree_set::BTreeSet<ProposalIdType> =
				EnactmentQueue::<T>::iter().flat_map(|(_, ids)| ids.into_iter()).collect();

			for (proposal_id, proposal) in Proposals::<T>::iter() {
				if proposal.state == ProposalState::Approved {
					assert!(
						queued_ids.contains(&proposal_id),
						"Approved proposal {proposal_id} missing from enactment queue"
					);
				}
			}
			Ok(())
		}
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::*;
	use encointer_primitives::democracy::{ProposalActionIdentifier, ProposalState};
	use frame_support::{assert_storage_noop, traits::OnRuntimeUpgrade};
	use mock::{new_test_ext, TestRuntime};

	#[allow(deprecated)]
	#[test]
	fn migration_v0_to_v1_works() {
		new_test_ext().execute_with(|| {
			StorageVersion::new(0).put::<Pallet<TestRuntime>>();

			// Insert some values into the v0 storage:

			v0::CancelledAt::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				42,
			);
			Proposals::<TestRuntime>::insert(
				1,
				Proposal {
					start: 0,
					start_cindex: 0,
					action: ProposalAction::SetInactivityTimeout(42),
					state: ProposalState::Approved,
					electorate_size: 0,
				},
			);
			// Use v1 storage alias (old format: single ProposalIdType)
			v1_storage::EnactmentQueue::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				1,
			);

			assert_eq!(v0::CancelledAt::<TestRuntime>::iter_keys().count(), 1);
			assert_eq!(Proposals::<TestRuntime>::iter_keys().count(), 1);
			assert_eq!(v1_storage::EnactmentQueue::<TestRuntime>::iter_keys().count(), 1);

			// Migrate V0 to V1.
			let state = v1::MigrateV0toV1purging::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v1::MigrateV0toV1purging::<TestRuntime>::on_runtime_upgrade();
			v1::MigrateV0toV1purging::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.
			assert_eq!(v0::CancelledAt::<TestRuntime>::iter_keys().count(), 0);
			assert_eq!(Proposals::<TestRuntime>::iter_keys().count(), 0);
			assert_eq!(v1_storage::EnactmentQueue::<TestRuntime>::iter_keys().count(), 0);
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_v1_to_v1_is_noop() {
		new_test_ext().execute_with(|| {
			StorageVersion::new(1).put::<Pallet<TestRuntime>>();

			LastApprovedProposalForAction::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				(42, 43),
			);

			let state = v1::MigrateV0toV1purging::<TestRuntime>::pre_upgrade().unwrap();
			assert_storage_noop!(v1::MigrateV0toV1purging::<TestRuntime>::on_runtime_upgrade());
			v1::MigrateV0toV1purging::<TestRuntime>::post_upgrade(state).unwrap();
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_v1_to_v2_works() {
		new_test_ext().execute_with(|| {
			StorageVersion::new(1).put::<Pallet<TestRuntime>>();

			// Insert old-format enactment queue entry
			v1_storage::EnactmentQueue::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				1,
			);

			// Insert a proposal that is in the queue
			Proposals::<TestRuntime>::insert(
				1,
				Proposal {
					start: 0,
					start_cindex: 0,
					action: ProposalAction::SetInactivityTimeout(42),
					state: ProposalState::Approved,
					electorate_size: 0,
				},
			);

			// Insert a proposal that was approved but NOT in the queue (the bug victim)
			Proposals::<TestRuntime>::insert(
				2,
				Proposal {
					start: 0,
					start_cindex: 0,
					action: ProposalAction::SetInactivityTimeout(99),
					state: ProposalState::Approved,
					electorate_size: 0,
				},
			);

			// Insert a non-approved proposal (should NOT be added to queue)
			Proposals::<TestRuntime>::insert(
				3,
				Proposal {
					start: 0,
					start_cindex: 0,
					action: ProposalAction::SetInactivityTimeout(50),
					state: ProposalState::Ongoing,
					electorate_size: 0,
				},
			);

			let state = v2::MigrateV1toV2::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v2::MigrateV1toV2::<TestRuntime>::on_runtime_upgrade();
			v2::MigrateV1toV2::<TestRuntime>::post_upgrade(state).unwrap();

			assert_eq!(Pallet::<TestRuntime>::on_chain_storage_version(), 2);

			// Both approved proposals should be in the queue
			let queue =
				EnactmentQueue::<TestRuntime>::get(ProposalActionIdentifier::SetInactivityTimeout)
					.unwrap();
			assert_eq!(queue.len(), 2);
			assert!(queue.contains(&1));
			assert!(queue.contains(&2));
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_v2_to_v2_is_noop() {
		new_test_ext().execute_with(|| {
			StorageVersion::new(2).put::<Pallet<TestRuntime>>();

			let queue = BoundedVec::try_from(vec![1u128]).unwrap();
			EnactmentQueue::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				queue,
			);

			assert_storage_noop!(v2::MigrateV1toV2::<TestRuntime>::on_runtime_upgrade());
		});
	}
}
