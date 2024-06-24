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
				"Running migration with current storage version {:?} / onchain {:?}",
				current_version,
				onchain_version
			);

			if onchain_version >= 1 {
				log::warn!(
					target: TARGET,
					"skipping on_runtime_upgrade: executed on wrong storage version."
				);
				return T::DbWeight::get().reads(1)
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
			EnactmentQueue::<TestRuntime>::insert(
				ProposalActionIdentifier::SetInactivityTimeout,
				1,
			);

			assert_eq!(v0::CancelledAt::<TestRuntime>::iter_keys().count(), 1);
			assert_eq!(Proposals::<TestRuntime>::iter_keys().count(), 1);
			assert_eq!(EnactmentQueue::<TestRuntime>::iter_keys().count(), 1);

			// Migrate V0 to V1.
			let state = v1::MigrateV0toV1purging::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v1::MigrateV0toV1purging::<TestRuntime>::on_runtime_upgrade();
			v1::MigrateV0toV1purging::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.
			assert_eq!(v0::CancelledAt::<TestRuntime>::iter_keys().count(), 0);
			assert_eq!(Proposals::<TestRuntime>::iter_keys().count(), 0);
			assert_eq!(EnactmentQueue::<TestRuntime>::iter_keys().count(), 0);
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
}
