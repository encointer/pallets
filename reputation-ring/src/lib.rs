// Copyright (c) 2019 Alain Brenzikofer
// This file is part of Encointer
//
// Encointer is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Encointer is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Encointer.  If not, see <http://www.gnu.org/licenses/>.

//! # Encointer Reputation Ring Pallet
//!
//! Bandersnatch key registration and per-community reputation ring publication.
//! After each ceremony cycle, computes 5 nested rings per community (N/5 for N=1..5)
//! where the N/5 ring contains all accounts that attended >= N of the last 5 ceremonies
//! in that community and have a registered Bandersnatch key.
//!
//! Ring computation is split across multiple blocks via `initiate_rings` +
//! `continue_ring_computation` to stay within block weight limits.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;
use encointer_primitives::{
	communities::CommunityIdentifier,
	scheduler::CeremonyIndexType,
};
use frame_support::{pallet_prelude::*, traits::Get};
use pallet_encointer_communities::Pallet as CommunitiesPallet;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod weights;
pub use weights::WeightInfo;

/// Maximum number of reputation levels (N=1..5).
pub const MAX_REPUTATION_LEVELS: u8 = 5;

/// Bandersnatch public key: 32 bytes.
pub type BandersnatchPublicKey = [u8; 32];

/// State machine for multi-block ring computation.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum RingComputationPhase {
	/// Collecting eligible members: scanning ceremony indices one by one.
	/// `next_ceremony_offset` tracks how many of the last 5 ceremonies have been scanned.
	CollectingMembers { next_ceremony_offset: u8 },
	/// Building ring for a given reputation level (1..=5).
	/// Members have been collected; now building rings from strictest (5/5) to loosest (1/5).
	BuildingRing { current_level: u8 },
	/// All rings computed. Ready to finalize.
	Done,
}

/// Pending ring computation state, stored on-chain during multi-block computation.
/// Uses `Vec` (unbounded) since this is transient state cleared after computation completes.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct RingComputationState<AccountId: Encode + Decode + Clone + Ord> {
	pub community: CommunityIdentifier,
	pub ceremony_index: CeremonyIndexType,
	pub phase: RingComputationPhase,
	/// Per-account attendance count (how many of last 5 ceremonies attended).
	/// Only accounts with registered Bandersnatch keys are included.
	/// Sorted by account for determinism.
	pub attendance: Vec<(AccountId, u8)>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_encointer_ceremonies::Config
		+ pallet_encointer_communities::Config
		+ pallet_encointer_scheduler::Config
	{
		#[allow(deprecated)]
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type WeightInfo: WeightInfo;

		/// Max members per ring.
		#[pallet::constant]
		type MaxRingSize: Get<u32>;

		/// Number of ceremony attendance records to process per block during member collection.
		#[pallet::constant]
		type ChunkSize: Get<u32>;
	}

	// -- Storage --

	/// Bandersnatch public key per account (registered once, updatable).
	#[pallet::storage]
	#[pallet::getter(fn bandersnatch_key)]
	pub type BandersnatchKeys<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BandersnatchPublicKey, OptionQuery>;

	/// Ordered member list (Bandersnatch pubkeys) per (community, ceremony_index, reputation_level).
	/// The N/5 ring contains pubkeys of accounts that attended >= N of the last 5 ceremonies.
	#[pallet::storage]
	#[pallet::getter(fn ring_members)]
	pub type RingMembers<T: Config> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, CommunityIdentifier>,
			NMapKey<Blake2_128Concat, CeremonyIndexType>,
			NMapKey<Blake2_128Concat, u8>,
		),
		BoundedVec<BandersnatchPublicKey, T::MaxRingSize>,
		OptionQuery,
	>;

	/// Multi-block computation state. Only one computation can be active at a time.
	/// Unbounded because this is transient state cleared after computation completes.
	#[pallet::storage]
	#[pallet::unbounded]
	#[pallet::getter(fn pending_ring_computation)]
	pub type PendingRingComputation<T: Config> =
		StorageValue<_, RingComputationState<T::AccountId>, OptionQuery>;

	// -- Events --

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A Bandersnatch key was registered or updated.
		BandersnatchKeyRegistered { account: T::AccountId, key: BandersnatchPublicKey },
		/// Ring computation started for a community at a ceremony index.
		RingComputationStarted {
			community: CommunityIdentifier,
			ceremony_index: CeremonyIndexType,
		},
		/// A ring was published for a specific reputation level.
		RingPublished {
			community: CommunityIdentifier,
			ceremony_index: CeremonyIndexType,
			reputation_level: u8,
			member_count: u32,
		},
		/// All 5 rings for a community/ceremony have been computed.
		RingComputationCompleted {
			community: CommunityIdentifier,
			ceremony_index: CeremonyIndexType,
		},
		/// A chunk of member collection was processed.
		MemberCollectionProgress {
			community: CommunityIdentifier,
			ceremony_index: CeremonyIndexType,
			ceremonies_scanned: u8,
		},
	}

	// -- Errors --

	#[pallet::error]
	pub enum Error<T> {
		/// A ring computation is already in progress.
		ComputationAlreadyInProgress,
		/// No ring computation is currently pending.
		NoComputationPending,
		/// The specified community does not exist.
		CommunityNotFound,
		/// The ceremony index is invalid (zero or in the future).
		InvalidCeremonyIndex,
		/// Ring computation is already complete; call finalize or start a new one.
		ComputationAlreadyDone,
		/// The ring exceeds MaxRingSize.
		RingTooLarge,
	}

	// -- Extrinsics --

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register or update a Bandersnatch public key for the caller.
		#[pallet::call_index(0)]
		#[pallet::weight(T::DbWeight::get().reads_writes(1, 1))]
		pub fn register_bandersnatch_key(
			origin: OriginFor<T>,
			key: BandersnatchPublicKey,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<BandersnatchKeys<T>>::insert(&who, key);
			Self::deposit_event(Event::BandersnatchKeyRegistered { account: who, key });
			Ok(().into())
		}

		/// Initiate ring computation for a community at a given ceremony index.
		/// Computes all 5 reputation rings (N/5 for N=1..5) via multi-block process.
		///
		/// The ceremony must have already occurred (ceremony_index < current).
		#[pallet::call_index(1)]
		#[pallet::weight(T::DbWeight::get().reads_writes(3, 1))]
		pub fn initiate_rings(
			origin: OriginFor<T>,
			community: CommunityIdentifier,
			ceremony_index: CeremonyIndexType,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			ensure!(
				!<PendingRingComputation<T>>::exists(),
				Error::<T>::ComputationAlreadyInProgress
			);

			// Validate community exists.
			ensure!(
				CommunitiesPallet::<T>::community_identifiers().contains(&community),
				Error::<T>::CommunityNotFound
			);

			// Validate ceremony index: must be > 0 and < current.
			let current_cindex =
				<pallet_encointer_scheduler::Pallet<T>>::current_ceremony_index();
			ensure!(
				ceremony_index > 0 && ceremony_index < current_cindex,
				Error::<T>::InvalidCeremonyIndex
			);

			let state = RingComputationState {
				community,
				ceremony_index,
				phase: RingComputationPhase::CollectingMembers { next_ceremony_offset: 0 },
				attendance: Vec::new(),
			};

			<PendingRingComputation<T>>::put(state);

			Self::deposit_event(Event::RingComputationStarted { community, ceremony_index });

			Ok(().into())
		}

		/// Continue the pending ring computation. Processes one chunk of work per call.
		///
		/// During member collection: scans one past ceremony's reputation records.
		/// During ring building: builds one ring level and stores it.
		///
		/// Can be called by anyone (intended for `on_idle` or off-chain worker).
		#[pallet::call_index(2)]
		#[pallet::weight(T::DbWeight::get().reads_writes(
			T::ChunkSize::get() as u64 + 2,
			T::ChunkSize::get() as u64 + 1
		))]
		pub fn continue_ring_computation(
			origin: OriginFor<T>,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			let mut state = <PendingRingComputation<T>>::get()
				.ok_or(Error::<T>::NoComputationPending)?;

			ensure!(
				state.phase != RingComputationPhase::Done,
				Error::<T>::ComputationAlreadyDone
			);

			Self::process_computation_step(&mut state)?;

			if state.phase == RingComputationPhase::Done {
				Self::deposit_event(Event::RingComputationCompleted {
					community: state.community,
					ceremony_index: state.ceremony_index,
				});
				<PendingRingComputation<T>>::kill();
			} else {
				<PendingRingComputation<T>>::put(state);
			}

			Ok(().into())
		}
	}
}

// -- Implementation --

impl<T: Config> Pallet<T> {
	/// Process one step of the multi-block ring computation.
	fn process_computation_step(
		state: &mut RingComputationState<T::AccountId>,
	) -> DispatchResult {
		match state.phase {
			RingComputationPhase::CollectingMembers { next_ceremony_offset } => {
				Self::collect_members_step(state, next_ceremony_offset)?;
			},
			RingComputationPhase::BuildingRing { current_level } => {
				Self::build_ring_step(state, current_level)?;
			},
			RingComputationPhase::Done => {},
		}
		Ok(())
	}

	/// Scan one past ceremony and update attendance counts.
	///
	/// For each ceremony offset (0..5), iterate over all accounts with registered
	/// Bandersnatch keys and check if they have verified reputation for this ceremony.
	fn collect_members_step(
		state: &mut RingComputationState<T::AccountId>,
		offset: u8,
	) -> DispatchResult {
		if offset >= MAX_REPUTATION_LEVELS {
			// All 5 ceremonies scanned. Sort attendance by account for deterministic ordering.
			state.attendance.sort_by(|a, b| a.0.cmp(&b.0));
			// Transition to ring building phase, starting from strictest (5/5).
			state.phase = RingComputationPhase::BuildingRing {
				current_level: MAX_REPUTATION_LEVELS,
			};
			return Ok(());
		}

		let cindex = state.ceremony_index.saturating_sub(offset as u32);
		if cindex == 0 {
			// No ceremony at index 0; skip.
			state.phase = RingComputationPhase::CollectingMembers {
				next_ceremony_offset: offset + 1,
			};
			Self::deposit_event(Event::MemberCollectionProgress {
				community: state.community,
				ceremony_index: state.ceremony_index,
				ceremonies_scanned: offset + 1,
			});
			return Ok(());
		}

		// Iterate over all accounts with registered Bandersnatch keys and check
		// their reputation via the public getter.
		use pallet_encointer_ceremonies::Pallet as CeremoniesPallet;
		for (account, _key) in <BandersnatchKeys<T>>::iter() {
			let reputation =
				CeremoniesPallet::<T>::participant_reputation((state.community, cindex), &account);
			if !reputation.is_verified() {
				continue;
			}
			// Update attendance count.
			if let Some(entry) = state.attendance.iter_mut().find(|(a, _)| *a == account) {
				entry.1 = entry.1.saturating_add(1);
			} else {
				state.attendance.push((account, 1));
			}
		}

		state.phase = RingComputationPhase::CollectingMembers {
			next_ceremony_offset: offset + 1,
		};

		Self::deposit_event(Event::MemberCollectionProgress {
			community: state.community,
			ceremony_index: state.ceremony_index,
			ceremonies_scanned: offset + 1,
		});

		Ok(())
	}

	/// Build one ring level and store the member list.
	///
	/// Builds from strictest (5/5) down to loosest (1/5).
	/// The N/5 ring contains all accounts with attendance >= N.
	fn build_ring_step(
		state: &mut RingComputationState<T::AccountId>,
		level: u8,
	) -> DispatchResult {
		if level == 0 {
			state.phase = RingComputationPhase::Done;
			return Ok(());
		}

		// Collect Bandersnatch pubkeys for accounts with attendance >= level.
		let mut members: Vec<BandersnatchPublicKey> = Vec::new();
		for (account, count) in state.attendance.iter() {
			if *count >= level {
				if let Some(key) = <BandersnatchKeys<T>>::get(account) {
					members.push(key);
				}
			}
		}

		// Sort for deterministic ordering.
		members.sort();

		let member_count = members.len() as u32;
		let bounded: BoundedVec<BandersnatchPublicKey, T::MaxRingSize> =
			BoundedVec::try_from(members).map_err(|_| Error::<T>::RingTooLarge)?;

		<RingMembers<T>>::insert(
			(state.community, state.ceremony_index, level),
			bounded,
		);

		Self::deposit_event(Event::RingPublished {
			community: state.community,
			ceremony_index: state.ceremony_index,
			reputation_level: level,
			member_count,
		});

		// Move to next level (4/5, 3/5, ..., 1/5, then done).
		state.phase = if level == 1 {
			RingComputationPhase::Done
		} else {
			RingComputationPhase::BuildingRing { current_level: level - 1 }
		};

		Ok(())
	}
}
