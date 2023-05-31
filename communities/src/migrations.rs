use super::*;
use frame_support::{pallet_prelude::*, storage_alias, traits::OnRuntimeUpgrade};

/// The log target.
const TARGET: &str = "communities::migration::v1";

mod v0 {
	use super::*;

	#[storage_alias]
	pub type CommunityIdentifiers<T: Config> =
		StorageValue<Pallet<T>, Vec<CommunityIdentifier>, ValueQuery>;

	#[storage_alias]
	pub(super) type CommunityIdentifiersByGeohash<T: Config> =
		StorageMap<Pallet<T>, Identity, GeoHash, Vec<CommunityIdentifier>, ValueQuery>;

	#[storage_alias]
	pub(super) type Locations<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		CommunityIdentifier,
		Identity,
		GeoHash,
		Vec<Location>,
		ValueQuery,
	>;

	#[storage_alias]
	pub(super) type Bootstrappers<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		CommunityIdentifier,
		Vec<<T as frame_system::Config>::AccountId>,
		ValueQuery,
	>;

	#[storage_alias]
	pub(super) type CommunityMetadata<T: Config> = StorageMap<
		Pallet<T>,
		Blake2_128Concat,
		CommunityIdentifier,
		CommunityMetadataType,
		ValueQuery,
	>;
}

pub mod v1 {
	use super::*;

	pub struct Migration<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for Migration<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "can only upgrade from version 0");

			let cid_count = v0::CommunityIdentifiers::<T>::get().len() as u32;
			log::info!(target: TARGET, "{} cids will be migrated.", cid_count,);
			ensure!(cid_count <= T::MaxCommunityIdentifiers::get(), "too many cids");

			let cids_by_geohash = v0::CommunityIdentifiersByGeohash::<T>::iter();
			let mut cids_by_geohash_count = 0u32;
			for cids in cids_by_geohash {
				let count = cids.1.len() as u32;
				ensure!(
					count <= T::MaxCommunityIdentifiersPerGeohash::get(),
					"too many cids per geohash"
				);
				cids_by_geohash_count = cids_by_geohash_count + count;
			}
			log::info!(
				target: TARGET,
				"{} cids by geohash will be migrated.",
				cids_by_geohash_count,
			);

			let locations_by_geohash = v0::Locations::<T>::iter();
			let mut locations_by_geohash_count = 0u32;
			for locations in locations_by_geohash {
				let count = locations.2.len() as u32;
				ensure!(
					count <= T::MaxLocationsPerGeohash::get(),
					"too many locations per geohash"
				);
				locations_by_geohash_count = locations_by_geohash_count + count;
			}
			log::info!(
				target: TARGET,
				"{} locations by geohash will be migrated.",
				locations_by_geohash_count,
			);

			let bootstrappers = v0::Bootstrappers::<T>::iter();
			let mut bootstrappers_count = 0u32;
			for bs in bootstrappers {
				let count = bs.1.len() as u32;
				ensure!(count <= T::MaxBootstrappers::get(), "too many bootstrappers");
				bootstrappers_count = bootstrappers_count + count
			}
			log::info!(target: TARGET, "{} bootstrappers will be migrated.", bootstrappers_count,);

			// For community metadata, we do not need any checks, because the data is bounded already due to the CommmunityMetadata validate() function.

			Ok((cid_count, cids_by_geohash_count, locations_by_geohash_count, bootstrappers_count)
				.encode())
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

			let (
				old_cids_count,
				old_cids_by_geohash_count,
				old_locations_by_geohash_count,
				old_bootstrappers_count,
			): (u32, u32, u32, u32) =
				Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

			let new_cids_count = crate::CommunityIdentifiers::<T>::get().len() as u32;
			assert_eq!(old_cids_count, new_cids_count, "must migrate all community identifiers");

			let new_cids_by_geohash_count =
				CommunityIdentifiersByGeohash::<T>::iter().fold(0, |acc, x| acc + x.1.len()) as u32;

			assert_eq!(
				old_cids_by_geohash_count, new_cids_by_geohash_count,
				"must migrate all community identifiers"
			);

			let new_locations_by_geohash_count =
				crate::Locations::<T>::iter().fold(0, |acc, x| acc + x.2.len()) as u32;

			assert_eq!(
				old_locations_by_geohash_count, new_locations_by_geohash_count,
				"must migrate all locations"
			);

			let new_bootstrappers_count =
				crate::Bootstrappers::<T>::iter().fold(0, |acc, x| acc + x.1.len()) as u32;

			assert_eq!(
				old_bootstrappers_count, new_bootstrappers_count,
				"must migrate all bootstrappers"
			);

			log::info!(target: TARGET, "{} community identifiers migrated", new_cids_count);
			log::info!(
				target: TARGET,
				"{} community identifiers by geohash migrated",
				new_cids_by_geohash_count
			);
			log::info!(target: TARGET, "{} locations migrated", new_locations_by_geohash_count);
			log::info!(target: TARGET, "{} bootstrappers migrated", new_bootstrappers_count);
			Ok(())
		}
	}
}

#[cfg(test)]
#[cfg(feature = "try-runtime")]
mod test {
	use super::*;
	use encointer_primitives::common::FromStr as PrimitivesFromStr;
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

			v0::CommunityIdentifiers::<TestRuntime>::put(cids.clone());

			let cids_by_geohash_0 = vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
			];

			v0::CommunityIdentifiersByGeohash::<TestRuntime>::insert(
				GeoHash::try_from("u0qjd").unwrap(),
				cids_by_geohash_0.clone(),
			);

			let cids_by_geohash_1 = vec![CommunityIdentifier::from_str("555552Fvv9e").unwrap()];

			v0::CommunityIdentifiersByGeohash::<TestRuntime>::insert(
				GeoHash::try_from("u0qje").unwrap(),
				cids_by_geohash_1.clone(),
			);

			let locations_0 = vec![
				Location::new(Degree::from_num(0), Degree::from_num(0)),
				Location::new(Degree::from_num(1), Degree::from_num(1)),
			];

			v0::Locations::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				GeoHash::try_from("u0qje").unwrap(),
				locations_0.clone(),
			);

			let locations_1 = vec![
				Location::new(Degree::from_num(2), Degree::from_num(2)),
				Location::new(Degree::from_num(3), Degree::from_num(3)),
				Location::new(Degree::from_num(4), Degree::from_num(4)),
			];

			v0::Locations::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
				GeoHash::try_from("u0qjd").unwrap(),
				locations_1.clone(),
			);

			let bootstrappers_0 =
				vec![AccountId::from(AccountKeyring::Alice), AccountId::from(AccountKeyring::Bob)];
			v0::Bootstrappers::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				bootstrappers_0.clone(),
			);

			let bootstrappers_1 = vec![
				AccountId::from(AccountKeyring::Ferdie),
				AccountId::from(AccountKeyring::Dave),
			];
			v0::Bootstrappers::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
				bootstrappers_1.clone(),
			);

			v0::CommunityMetadata::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
				CommunityMetadataType {
					name: "AName".into(),
					symbol: "ASY".into(),
					assets: "Defau1tCidThat1s46Characters1nLength1111111111".into(),
					theme: None,
					url: Some("AUrl".into()),
				},
			);

			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v1::Migration::<TestRuntime>::on_runtime_upgrade();
			v1::Migration::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.

			assert_eq!(
				crate::CommunityIdentifiers::<TestRuntime>::get(),
				BoundedVec::<
					CommunityIdentifier,
					<TestRuntime as Config>::MaxCommunityIdentifiers,
				>::try_from(cids)
				.unwrap()
			);

			assert_eq!(
				crate::CommunityIdentifiersByGeohash::<TestRuntime>::get(
					GeoHash::try_from("u0qjd").unwrap(),
				),
				BoundedVec::<
					CommunityIdentifier,
					<TestRuntime as Config>::MaxCommunityIdentifiersPerGeohash,
				>::try_from(cids_by_geohash_0)
				.unwrap()
			);

			assert_eq!(
				crate::CommunityIdentifiersByGeohash::<TestRuntime>::get(
					GeoHash::try_from("u0qje").unwrap(),
				),
				BoundedVec::<
					CommunityIdentifier,
					<TestRuntime as Config>::MaxCommunityIdentifiersPerGeohash,
				>::try_from(cids_by_geohash_1)
				.unwrap()
			);

			assert_eq!(
				crate::Locations::<TestRuntime>::get(
					CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
					GeoHash::try_from("u0qje").unwrap(),
				),
				BoundedVec::<Location, <TestRuntime as Config>::MaxLocationsPerGeohash>::try_from(
					locations_0
				)
				.unwrap()
			);

			assert_eq!(
				crate::Locations::<TestRuntime>::get(
					CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
					GeoHash::try_from("u0qjd").unwrap(),
				),
				BoundedVec::<Location, <TestRuntime as Config>::MaxLocationsPerGeohash>::try_from(
					locations_1
				)
				.unwrap()
			);

			assert_eq!(
				crate::Bootstrappers::<TestRuntime>::get(
					CommunityIdentifier::from_str("111112Fvv9e").unwrap()
				),
				BoundedVec::<
					<TestRuntime as frame_system::Config>::AccountId,
					<TestRuntime as Config>::MaxLocationsPerGeohash,
				>::try_from(bootstrappers_0)
				.unwrap()
			);

			assert_eq!(
				crate::Bootstrappers::<TestRuntime>::get(
					CommunityIdentifier::from_str("111112Fvv9d").unwrap()
				),
				BoundedVec::<
					<TestRuntime as frame_system::Config>::AccountId,
					<TestRuntime as Config>::MaxLocationsPerGeohash,
				>::try_from(bootstrappers_1)
				.unwrap()
			);

			assert_eq!(
				crate::CommunityMetadata::<TestRuntime>::get(
					CommunityIdentifier::from_str("111112Fvv9d").unwrap()
				),
				BoundedCommunityMetadata {
					name: BoundedPalletString::from_str("AName").unwrap(),
					symbol: BoundedPalletString::from_str("ASY").unwrap(),
					assets: BoundedPalletString::from_str(
						"Defau1tCidThat1s46Characters1nLength1111111111"
					)
					.unwrap(),
					theme: None,
					url: Some(BoundedPalletString::from_str("AUrl").unwrap()),
				}
			);
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_fails_with_too_many_cids() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			let cids = vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
			];

			v0::CommunityIdentifiers::<TestRuntime>::put(cids);
			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade();
			assert_err!(state, "too many cids");
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_fails_with_too_many_cids_per_geohash() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			let cids = vec![
				CommunityIdentifier::from_str("111112Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
				CommunityIdentifier::from_str("333332Fvv9e").unwrap(),
			];

			v0::CommunityIdentifiersByGeohash::<TestRuntime>::insert(
				GeoHash::try_from("u0qje").unwrap(),
				cids,
			);
			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade();
			assert_err!(state, "too many cids per geohash");
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_fails_with_too_many_locations() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			let locations = vec![Location::new(Degree::from_num(2), Degree::from_num(2)); 201];

			v0::Locations::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
				GeoHash::try_from("u0qjd").unwrap(),
				locations,
			);
			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade();
			assert_err!(state, "too many locations per geohash");
		});
	}

	#[allow(deprecated)]
	#[test]
	fn migration_fails_with_too_many_bootstrappers() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:
			let bootstrappers = vec![
				AccountId::from(AccountKeyring::Ferdie),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
				AccountId::from(AccountKeyring::Dave),
			];
			v0::Bootstrappers::<TestRuntime>::insert(
				CommunityIdentifier::from_str("111112Fvv9d").unwrap(),
				bootstrappers.clone(),
			);
			// Migrate.
			let state = v1::Migration::<TestRuntime>::pre_upgrade();
			assert_err!(state, "too many bootstrappers");
		});
	}
}
