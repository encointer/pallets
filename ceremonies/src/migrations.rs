use super::*;
use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};

/// The log target.
const TARGET: &str = "ceremonies::migration::v1";

mod v0 {
	use super::*;

	#[storage_alias]
	pub(super) type AttestationRegistry<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		AttestationIndexType,
		Vec<<T as frame_system::Config>::AccountId>,
		OptionQuery,
	>;
}

pub mod v1 {
	use super::*;

	pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for MigrateToV1<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "can only upgrade from version 0");

			let attestations = v0::AttestationRegistry::<T>::iter();
			let mut attestation_count = 0u32;
			for a in attestations {
				let count = a.2.len() as u32;
				ensure!(count <= T::MaxAttestations::get(), "too many attestations");
				attestation_count = attestation_count + count;
			}
			log::info!(target: TARGET, "{} attestations will be migrated.", attestation_count,);

			Ok((attestation_count).encode())
		}

		fn on_runtime_upgrade() -> Weight {
			let weight = T::DbWeight::get().reads(1);
			if StorageVersion::get::<Pallet<T>>() != 0 {
				log::warn!(
					target: TARGET,
					"skipping on_runtime_upgrade: executed on wrong storage version.\
				Expected version 0"
				);
				return weight
			}

			// we do not actually migrate any data, because it seems that the storage representation of Vec and BoundedVec is the same.
			// as long as we check the bounds in pre_upgrade, we should be fine.

			StorageVersion::new(1).put::<Pallet<T>>();
			weight.saturating_add(T::DbWeight::get().reads_writes(1, 2))
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "must upgrade");

			let old_attestation_count: u32 =
				Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

			let new_attestation_count =
				crate::AttestationRegistry::<T>::iter().fold(0, |acc, x| acc + x.2.len()) as u32;

			assert_eq!(
				old_attestation_count, new_attestation_count,
				"must migrate all attestations"
			);

			log::info!(target: TARGET, "{} attestations migrated", new_attestation_count);
			Ok(())
		}
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::*;
	use frame_support::{assert_err, traits::OnRuntimeUpgrade};
	use mock::{new_test_ext, TestRuntime};
	use sp_std::str::FromStr;
	use test_utils::*;
	#[allow(deprecated)]
	#[test]
	fn migration_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:

			let cids = vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
			];

			let attestations_0 =
				vec![AccountId::from(AccountKeyring::Alice), AccountId::from(AccountKeyring::Bob)];
			let attestations_1 = vec![
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Ferdie),
				AccountId::from(AccountKeyring::Charlie),
			];

			let attestations_2 = vec![
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Ferdie),
				AccountId::from(AccountKeyring::Charlie),
			];

			v0::AttestationRegistry::<TestRuntime>::insert((cids[0], 1), 0, attestations_0.clone());

			v0::AttestationRegistry::<TestRuntime>::insert((cids[0], 2), 3, attestations_1.clone());

			v0::AttestationRegistry::<TestRuntime>::insert((cids[0], 1), 3, attestations_1.clone());

			v0::AttestationRegistry::<TestRuntime>::insert((cids[1], 2), 3, attestations_2.clone());

			// Migrate.
			let state = v1::MigrateToV1::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v1::MigrateToV1::<TestRuntime>::on_runtime_upgrade();
			v1::MigrateToV1::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.

			assert_eq!(
				crate::AttestationRegistry::<TestRuntime>::get((cids[0], 1), 0),
				Some(
					BoundedVec::<
						<TestRuntime as frame_system::Config>::AccountId,
						<TestRuntime as Config>::MaxAttestations,
					>::try_from(attestations_0)
					.unwrap()
				)
			);

			assert_eq!(
				crate::AttestationRegistry::<TestRuntime>::get((cids[0], 2), 3),
				Some(
					BoundedVec::<
						<TestRuntime as frame_system::Config>::AccountId,
						<TestRuntime as Config>::MaxAttestations,
					>::try_from(attestations_1.clone())
					.unwrap()
				)
			);

			assert_eq!(
				crate::AttestationRegistry::<TestRuntime>::get((cids[0], 1), 3),
				Some(
					BoundedVec::<
						<TestRuntime as frame_system::Config>::AccountId,
						<TestRuntime as Config>::MaxAttestations,
					>::try_from(attestations_1)
					.unwrap()
				)
			);

			assert_eq!(
				crate::AttestationRegistry::<TestRuntime>::get((cids[1], 2), 3),
				Some(
					BoundedVec::<
						<TestRuntime as frame_system::Config>::AccountId,
						<TestRuntime as Config>::MaxAttestations,
					>::try_from(attestations_2)
					.unwrap()
				)
			);
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_fails_with_too_many_attestations() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			let attestations = vec![
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
				AccountId::from(AccountKeyring::Alice),
			];

			v0::AttestationRegistry::<TestRuntime>::insert(
				(CommunityIdentifier::from_str("111112Fvv9d").unwrap(), 1),
				0,
				attestations,
			);
			// Migrate.
			let state = v1::MigrateToV1::<TestRuntime>::pre_upgrade();
			assert_err!(state, "too many attestations");
		});
	}
}
