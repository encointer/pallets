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

	#[derive(
		Default,
		Encode,
		Decode,
		DecodeWithMemTracking,
		Copy,
		Clone,
		PartialEq,
		Eq,
		RuntimeDebug,
		TypeInfo,
		MaxEncodedLen,
	)]
	pub enum Reputation {
		// no attestations for attendance claim
		#[default]
		Unverified,
		// no attestation yet but linked to reputation
		UnverifiedReputable,
		// verified former attendance that has not yet been linked to a new registration
		VerifiedUnlinked,
		// verified former attendance that has already been linked to a new registration
		VerifiedLinked,
	}

	impl Reputation {
		pub fn migrate_to_v2(
			self,
			linked_cindex: CeremonyIndexType,
		) -> encointer_primitives::ceremonies::Reputation {
			match self {
				Reputation::UnverifiedReputable => {
					encointer_primitives::ceremonies::Reputation::UnverifiedReputable
				},
				Reputation::VerifiedLinked => {
					encointer_primitives::ceremonies::Reputation::VerifiedLinked(linked_cindex)
				},
				Reputation::VerifiedUnlinked => {
					encointer_primitives::ceremonies::Reputation::VerifiedUnlinked
				},
				Reputation::Unverified => encointer_primitives::ceremonies::Reputation::Unverified,
			}
		}
	}

	#[storage_alias]
	pub(super) type ParticipantReputation<T: Config> = StorageDoubleMap<
		Pallet<T>,
		Blake2_128Concat,
		CommunityCeremony,
		Blake2_128Concat,
		<T as frame_system::Config>::AccountId,
		Reputation,
		ValueQuery,
	>;

	pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for MigrateToV1<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 0, "can only upgrade from version 0");

			let attestations = v0::AttestationRegistry::<T>::iter();
			let mut attestation_count = 0u32;
			for a in attestations {
				let count = a.2.len() as u32;
				ensure!(count <= T::MaxAttestations::get(), "too many attestations");
				attestation_count += count;
			}
			log::info!(target: TARGET, "{attestation_count} attestations will be migrated.");

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
				return weight;
			}

			// we do not actually migrate any data, because it seems that the storage representation
			// of Vec and BoundedVec is the same. as long as we check the bounds in pre_upgrade, we
			// should be fine.

			StorageVersion::new(1).put::<Pallet<T>>();
			weight.saturating_add(T::DbWeight::get().reads_writes(1, 2))
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "must upgrade");

			let old_attestation_count: u32 =
				Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

			let new_attestation_count =
				crate::AttestationRegistry::<T>::iter().fold(0, |acc, x| acc + x.2.len()) as u32;

			assert_eq!(
				old_attestation_count, new_attestation_count,
				"must migrate all attestations"
			);

			log::info!(target: TARGET, "{new_attestation_count} attestations migrated");
			Ok(())
		}
	}
}

pub mod v2 {
	use super::*;
	use sp_runtime::Saturating;

	pub struct MigrateToV2<T>(sp_std::marker::PhantomData<T>);

	impl<T: Config + frame_system::Config> OnRuntimeUpgrade for MigrateToV2<T> {
		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<Vec<u8>, DispatchError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "can only upgrade from version 1");

			let reputation_count = v1::ParticipantReputation::<T>::iter().count() as u32;
			log::info!(target: TARGET, "{reputation_count} reputation entries will be migrated.");

			Ok((reputation_count).encode())
		}

		fn on_runtime_upgrade() -> Weight {
			let weight = T::DbWeight::get().reads(1);
			if StorageVersion::get::<Pallet<T>>() != 1 {
				log::warn!(
					target: TARGET,
					"skipping on_runtime_upgrade: executed on wrong storage version.\
				Expected version 1"
				);
				return weight;
			}
			let cindex = pallet_encointer_scheduler::Pallet::<T>::current_ceremony_index();
			let phase = pallet_encointer_scheduler::Pallet::<T>::current_phase();
			let linked_cindex = match phase {
				CeremonyPhaseType::Attesting => cindex + 1,
				_ => cindex,
			};

			let mut translated = 0u64;
			ParticipantReputation::<T>::translate::<v1::Reputation, _>(
				|_cc: CommunityCeremony, _account: T::AccountId, rep: v1::Reputation| {
					translated.saturating_inc();
					Some(rep.migrate_to_v2(linked_cindex))
				},
			);
			StorageVersion::new(2).put::<Pallet<T>>();
			T::DbWeight::get().reads_writes(translated, translated + 1)
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(state: Vec<u8>) -> Result<(), DispatchError> {
			assert_eq!(StorageVersion::get::<Pallet<T>>(), 2, "must upgrade");

			let old_reputation_count: u32 =
				Decode::decode(&mut &state[..]).expect("pre_upgrade provides a valid state; qed");

			let new_reputation_count = crate::ParticipantReputation::<T>::iter().count() as u32;

			assert_eq!(old_reputation_count, new_reputation_count, "must migrate all reputations");

			log::info!(target: TARGET, "{new_reputation_count} reputation entries migrated");
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
	fn migration_to_v1_works() {
		new_test_ext().execute_with(|| {
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 0);
			// Insert some values into the v0 storage:

			let cids = [
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
	fn migration_to_v1_fails_with_too_many_attestations() {
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
	#[allow(deprecated)]
	#[test]
	fn migration_to_v2_works() {
		new_test_ext().execute_with(|| {
			StorageVersion::new(1).put::<Pallet<TestRuntime>>();
			assert_eq!(StorageVersion::get::<Pallet<TestRuntime>>(), 1);
			// Insert some values into the v0 storage:
			let alice: AccountId = AccountKeyring::Alice.into();
			let bob: AccountId = AccountKeyring::Bob.into();
			let old_rep = v1::Reputation::VerifiedLinked;
			let cid = CommunityIdentifier::from_str("111112Fvv9e").unwrap();
			let cid2 = CommunityIdentifier::from_str("111112Fvv9f").unwrap();

			v1::ParticipantReputation::<TestRuntime>::insert((cid, 1), alice.clone(), old_rep);
			v1::ParticipantReputation::<TestRuntime>::insert((cid, 2), bob.clone(), old_rep);
			v1::ParticipantReputation::<TestRuntime>::insert((cid2, 0), alice.clone(), old_rep);
			// Migrate.
			let state = v2::MigrateToV2::<TestRuntime>::pre_upgrade().unwrap();
			let _weight = v2::MigrateToV2::<TestRuntime>::on_runtime_upgrade();
			v2::MigrateToV2::<TestRuntime>::post_upgrade(state).unwrap();

			// Check that all values got migrated.

			assert_eq!(
				crate::ParticipantReputation::<TestRuntime>::get((cid, 1), alice.clone()),
				Reputation::VerifiedLinked(1)
			);
			assert_eq!(
				crate::ParticipantReputation::<TestRuntime>::get((cid, 2), bob),
				Reputation::VerifiedLinked(1)
			);
			assert_eq!(
				crate::ParticipantReputation::<TestRuntime>::get((cid2, 0), alice),
				Reputation::VerifiedLinked(1)
			);
		});
	}
}
