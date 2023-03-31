use super::*;
use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade, BoundedVec};

/// The log target.
const TARGET: &'static str = "communities::migration::v1";

mod v0 {
	use super::*;

	#[storage_alias]
	pub type CommunityIdentifiers<T: Config> =
		StorageValue<Pallet<T>, Vec<CommunityIdentifier>, ValueQuery>;
}

pub mod v1 {
	use super::*;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for Migration<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "can only upgrade from version 0");

			let cid_count = v0::CommunityIdentifiers::<T>::get().len();
			log::info!(target: TARGET, "{} cids will be migrated.", cid_count,);
			ensure!(cid_count <= T::MaxCommunityIdentifiers::get() as usize, "too many cids");

			Ok((cid_count as u32).encode())
		}

		#[allow(deprecated)]
		fn on_runtime_upgrade() -> Weight {
			let mut weight = T::DbWeight::get().reads(1);
			if StorageVersion::get::<Pallet<T>>() != 0 {
				log::warn!(
					target: TARGET,
					"skipping on_runtime_upgrade: executed on wrong storage version.\
				Expected version 0"
				);
				return weight
			}

			let cids = v0::CommunityIdentifiers::<T>::take();
			let bounded = BoundedVec::<_, T::MaxCommunityIdentifiers>::truncate_from(cids.clone());
			CommunityIdentifiers::<T>::put(bounded);
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));

			if cids.len() as u32 > T::MaxCommunityIdentifiers::get() {
				log::error!(
					target: TARGET,
					"truncated {} community identifiers to {}; continuing",
					cids.len(),
					T::MaxCommunityIdentifiers::get()
				);
			}

			StorageVersion::new(1).put::<Pallet<T>>();
			weight.saturating_add(T::DbWeight::get().reads_writes(1, 2))
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "must upgrade");

			let old_cids_count: u32 =
				Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");
			let new_cids_count = crate::CommunityIdentifiers::<T>::get().len() as u32;
			assert_eq!(old_cids_count, new_cids_count, "must migrate all community identifiers");

			log::info!(target: TARGET, "{} community identifiers migrated", new_cids_count,);
			Ok(())
		}
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::*;
	use frame_support::bounded_vec;
	use mock::{new_test_ext, TestRuntime};
	use sp_std::str::FromStr;

	#[allow(deprecated)]
	#[test]
	fn migration_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			v0::CommunityIdentifiers::<TestRuntime>::put(vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("555552Fvv9e").unwrap(),
			]);

			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v1::Migration::<TestRuntime>::on_runtime_upgrade();
			v1::Migration::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.

			// Case 3: Public proposals
			let cids: BoundedVec<_, <TestRuntime as Config>::MaxCommunityIdentifiers> = bounded_vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("555552Fvv9e").unwrap(),
			];
			assert_eq!(CommunityIdentifiers::<TestRuntime>::get(), cids);
		});
	}
}
