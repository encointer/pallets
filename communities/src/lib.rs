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

//! # Encointer Communities Module
//!
//! provides functionality for
//! - registering new communities
//! - modifying community characteristics

#![cfg_attr(not(feature = "std"), no_std)]

use core::marker::PhantomData;
use encointer_primitives::{
	balances::{BalanceEntry, Demurrage},
	common::PalletString,
	communities::{
		consts::*, CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Degree,
		GeoHash, Location, LossyFrom, NominalIncome as NominalIncomeType,
	},
	fixed::transcendental::{asin, cos, powi, sin, sqrt},
	scheduler::CeremonyPhaseType,
};
use frame_support::{ensure, pallet_prelude::DispatchResultWithPostInfo, BoundedVec};
use frame_system::pallet_prelude::BlockNumberFor;
use log::{info, warn};
use parity_scale_codec::Encode;
use sp_runtime::{traits::Get, DispatchResult, SaturatedConversion};
use sp_std::{prelude::*, result::Result};

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use encointer_primitives::communities::{MaxSpeedMpsType, MinSolarTripTimeType};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);
	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ pallet_encointer_scheduler::Config
		+ pallet_encointer_balances::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Required origin for updating a community (though can always be Root).
		type CommunityMaster: EnsureOrigin<Self::RuntimeOrigin>;
		/// Origin for non destructive actions like adding a community or location
		type TrustableForNonDestructiveAction: EnsureOrigin<Self::RuntimeOrigin>;

		type WeightInfo: WeightInfo;

		#[pallet::constant]
		type MaxCommunityIdentifiers: Get<u32>;

		#[pallet::constant]
		type MaxCommunityIdentifiersPerGeohash: Get<u32>;

		#[pallet::constant]
		type MaxLocationsPerGeohash: Get<u32>;

		#[pallet::constant]
		type MaxBootstrappers: Get<u32>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new community.
		///
		/// May only be called from `T::TrustableForNonDestructiveAction`.
		#[pallet::call_index(0)]
		#[pallet::weight((<T as Config>::WeightInfo::new_community(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn new_community(
			origin: OriginFor<T>,
			location: Location,
			bootstrappers: Vec<T::AccountId>,
			community_metadata: CommunityMetadataType,
			demurrage: Option<Demurrage>,
			nominal_income: Option<NominalIncomeType>,
		) -> DispatchResultWithPostInfo {
			T::TrustableForNonDestructiveAction::ensure_origin(origin)?;
			Self::validate_bootstrappers(&bootstrappers)?;
			community_metadata
				.validate()
				.map_err(|_| <Error<T>>::InvalidCommunityMetadata)?;

			let cid = CommunityIdentifier::new(location, bootstrappers.clone())
				.map_err(|_| Error::<T>::InvalidLocation)?;
			let cids = Self::community_identifiers();
			ensure!(!cids.contains(&cid), Error::<T>::CommunityAlreadyRegistered);

			Self::validate_location(&location)?;
			// All checks done, now mutate state
			let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
				.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
			let locations: BoundedVec<Location, T::MaxLocationsPerGeohash> =
				BoundedVec::try_from(vec![location])
					.map_err(|_| Error::<T>::TooManyLocationsPerGeohash)?;

			// insert cid into cids_by_geohash map
			let mut cids_in_bucket = Self::cids_by_geohash(&geo_hash);
			match cids_in_bucket.binary_search(&cid) {
				Ok(_) => (),
				Err(index) => {
					cids_in_bucket
						.try_insert(index, cid)
						.map_err(|_| Error::<T>::TooManyCommunityIdentifiersPerGeohash)?;
					<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids_in_bucket);
				},
			}

			// insert location into cid -> geohash -> location map
			<Locations<T>>::insert(cid, geo_hash, locations);

			CommunityIdentifiers::<T>::try_append(cid)
				.map_err(|_| Error::<T>::TooManyCommunityIdentifiers)?;

			<Bootstrappers<T>>::insert(
				cid,
				&BoundedVec::try_from(bootstrappers)
					.map_err(|_| Error::<T>::TooManyBootstrappers)?,
			);
			<CommunityMetadata<T>>::insert(cid, &community_metadata);

			if let Some(d) = demurrage {
				<pallet_encointer_balances::Pallet<T>>::set_demurrage(&cid, d)
					.map_err(|_| <Error<T>>::InvalidDemurrage)?;
			}
			if let Some(i) = nominal_income {
				<NominalIncome<T>>::insert(cid, i)
			}

			sp_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
			sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

			Self::deposit_event(Event::CommunityRegistered(cid));
			info!(target: LOG, "registered community with cid: {:?}", cid);
			Ok(().into())
		}

		/// Add a new meetup `location` to the community with `cid`.
		///
		/// May only be called from `T::TrustableForNonDestructiveAction`.
		///
		/// Todo: Replace `T::CommunityMaster` with community governance: #137.
		#[pallet::call_index(1)]
		#[pallet::weight((<T as Config>::WeightInfo::add_location(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn add_location(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			location: Location,
		) -> DispatchResultWithPostInfo {
			T::TrustableForNonDestructiveAction::ensure_origin(origin)?;
			ensure!(
				<pallet_encointer_scheduler::Pallet<T>>::current_phase() ==
					CeremonyPhaseType::Registering,
				Error::<T>::RegistrationPhaseRequired
			);
			Self::do_add_location(cid, location)
		}

		/// Remove an existing meetup `location` from the community with `cid`.
		///
		/// May only be called from `T::CommunityMaster`.
		///
		/// Todo: Replace `T::CommunityMaster` with community governance: #137.
		#[pallet::call_index(2)]
		#[pallet::weight((<T as Config>::WeightInfo::remove_location(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn remove_location(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			location: Location,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			ensure!(
				<pallet_encointer_scheduler::Pallet<T>>::current_phase() ==
					CeremonyPhaseType::Registering,
				Error::<T>::RegistrationPhaseRequired
			);
			Self::do_remove_location(cid, location)
		}

		/// Update the metadata of the community with `cid`.
		///
		/// May only be called from `T::CommunityMaster`.
		#[pallet::call_index(3)]
		#[pallet::weight((<T as Config>::WeightInfo::update_community_metadata(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn update_community_metadata(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			community_metadata: CommunityMetadataType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			Self::do_update_community_metadata(cid, community_metadata)
		}

		#[pallet::call_index(4)]
		#[pallet::weight((<T as Config>::WeightInfo::update_demurrage(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn update_demurrage(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			demurrage: Demurrage,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			Self::do_update_demurrage(cid, demurrage)
		}

		#[pallet::call_index(5)]
		#[pallet::weight((<T as Config>::WeightInfo::update_nominal_income(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn update_nominal_income(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			nominal_income: NominalIncomeType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			Self::do_update_nominal_income(cid, nominal_income)
		}

		#[pallet::call_index(6)]
		#[pallet::weight((<T as Config>::WeightInfo::set_min_solar_trip_time_s(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn set_min_solar_trip_time_s(
			origin: OriginFor<T>,
			min_solar_trip_time_s: MinSolarTripTimeType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			<MinSolarTripTimeS<T>>::put(min_solar_trip_time_s);
			info!(target: LOG, "set min solar trip time to {} s", min_solar_trip_time_s);
			Self::deposit_event(Event::MinSolarTripTimeSUpdated(min_solar_trip_time_s));
			Ok(().into())
		}

		#[pallet::call_index(7)]
		#[pallet::weight((<T as Config>::WeightInfo::set_max_speed_mps(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn set_max_speed_mps(
			origin: OriginFor<T>,
			max_speed_mps: MaxSpeedMpsType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			<MaxSpeedMps<T>>::put(max_speed_mps);
			info!(target: LOG, "set max speed mps to {}", max_speed_mps);
			Self::deposit_event(Event::MaxSpeedMpsUpdated(max_speed_mps));
			Ok(().into())
		}

		#[pallet::call_index(8)]
		#[pallet::weight((<T as Config>::WeightInfo::purge_community(), DispatchClass::Normal, Pays::Yes)
        )]
		pub fn purge_community(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			Self::remove_community(cid);
			Ok(().into())
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new community was registered [community_identifier]
		CommunityRegistered(CommunityIdentifier),
		/// CommunityMetadata was updated [community_identifier]
		MetadataUpdated(CommunityIdentifier),
		/// A community's nominal income was updated [community_identifier, new_income]
		NominalIncomeUpdated(CommunityIdentifier, NominalIncomeType),
		/// A community's demurrage was updated [community_identifier, new_demurrage]
		DemurrageUpdated(CommunityIdentifier, Demurrage),
		/// A location has been added
		LocationAdded(CommunityIdentifier, Location),
		/// A location has been removed
		LocationRemoved(CommunityIdentifier, Location),
		/// A security parameter for minimum meetup location distance has changed
		MinSolarTripTimeSUpdated(MinSolarTripTimeType),
		/// A security parameter for minimum meetup location distance has changed
		MaxSpeedMpsUpdated(MaxSpeedMpsType),
		/// a community has been purged
		CommunityPurged(CommunityIdentifier),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Location is not a valid geolocation
		InvalidLocation,
		/// Invalid amount of bootstrappers supplied. Needs to be \[3, 12\]
		InvalidAmountBootstrappers,
		/// minimum distance violation to other location
		MinimumDistanceViolationToOtherLocation,
		/// minimum distance violated towards dateline
		MinimumDistanceViolationToDateLine,
		/// Can't register community that already exists
		CommunityAlreadyRegistered,
		/// Community does not exist yet
		CommunityInexistent,
		/// Invalid Metadata supplied
		InvalidCommunityMetadata,
		/// Invalid demurrage supplied
		InvalidDemurrage,
		/// Invalid demurrage supplied
		InvalidNominalIncome,
		/// Invalid location provided when computing geohash
		InvalidLocationForGeohash,
		/// Invalid Geohash provided
		InvalidGeohash,
		/// sender is not authorized
		BadOrigin,
		/// Locations can only be added in Registration Phase
		RegistrationPhaseRequired,
		/// CommunityIdentifiers BoundedVec is full
		TooManyCommunityIdentifiers,
		/// CommunityIdentifiersPerGeohash BoundedVec is full
		TooManyCommunityIdentifiersPerGeohash,
		/// LocationsPerGeohash BoundedVec is full
		TooManyLocationsPerGeohash,
		/// Bootstrappers BoundedVec is full
		TooManyBootstrappers,
	}

	#[pallet::storage]
	#[pallet::getter(fn cids_by_geohash)]
	pub(super) type CommunityIdentifiersByGeohash<T: Config> = StorageMap<
		_,
		Identity,
		GeoHash,
		BoundedVec<CommunityIdentifier, T::MaxCommunityIdentifiersPerGeohash>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn locations)]
	pub(super) type Locations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Identity,
		GeoHash,
		BoundedVec<Location, T::MaxLocationsPerGeohash>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bootstrappers)]
	pub(super) type Bootstrappers<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		BoundedVec<T::AccountId, T::MaxBootstrappers>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn community_identifiers)]
	pub(super) type CommunityIdentifiers<T: Config> =
		StorageValue<_, BoundedVec<CommunityIdentifier, T::MaxCommunityIdentifiers>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn community_metadata)]
	pub(super) type CommunityMetadata<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdentifier, CommunityMetadataType, ValueQuery>;

	/// Amount of UBI to be paid for every attended ceremony.
	#[pallet::storage]
	#[pallet::getter(fn nominal_income)]
	pub type NominalIncome<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdentifier, NominalIncomeType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn min_solar_trip_time_s)]
	pub(super) type MinSolarTripTimeS<T: Config> =
		StorageValue<_, MinSolarTripTimeType, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn max_speed_mps)]
	pub(super) type MaxSpeedMps<T: Config> = StorageValue<_, MaxSpeedMpsType, ValueQuery>;

	#[derive(frame_support::DefaultNoBound)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T> {
		pub min_solar_trip_time_s: MinSolarTripTimeType,
		pub max_speed_mps: MaxSpeedMpsType,
		#[serde(skip)]
		pub _config: sp_std::marker::PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			<MinSolarTripTimeS<T>>::put(self.min_solar_trip_time_s);
			<MaxSpeedMps<T>>::put(self.max_speed_mps);
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_add_location(
		cid: CommunityIdentifier,
		location: Location,
	) -> DispatchResultWithPostInfo {
		Self::ensure_cid_exists(&cid)?;
		Self::validate_location(&location)?;
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
			.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
		// insert location into locations
		let mut locations = Self::locations(cid, &geo_hash);
		match locations.binary_search(&location) {
			Ok(_) => (),
			Err(index) => {
				locations
					.try_insert(index, location)
					.map_err(|_| Error::<T>::TooManyLocationsPerGeohash)?;
				<Locations<T>>::insert(cid, &geo_hash, locations);
			},
		}
		// check if cid is in cids_by_geohash, if not, add it
		let mut cids = Self::cids_by_geohash(&geo_hash);
		match cids.binary_search(&cid) {
			Ok(_) => (),
			Err(index) => {
				cids.try_insert(index, cid)
					.map_err(|_| Error::<T>::TooManyCommunityIdentifiersPerGeohash)?;
				<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids);
			},
		}
		sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

		info!(target: LOG, "added location {:?} to community with cid: {:?}", location, cid);
		Self::deposit_event(Event::LocationAdded(cid, location));
		Ok(().into())
	}

	pub fn do_remove_location(
		cid: CommunityIdentifier,
		location: Location,
	) -> DispatchResultWithPostInfo {
		Self::ensure_cid_exists(&cid)?;

		let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
			.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
		Self::remove_location_intern(cid, location, geo_hash);
		info!(target: LOG, "removed location {:?} to community with cid: {:?}", location, cid);
		Self::deposit_event(Event::LocationRemoved(cid, location));
		Ok(().into())
	}
	pub fn do_update_community_metadata(
		cid: CommunityIdentifier,
		community_metadata: CommunityMetadataType,
	) -> DispatchResultWithPostInfo {
		Self::ensure_cid_exists(&cid)?;
		community_metadata
			.validate()
			.map_err(|_| <Error<T>>::InvalidCommunityMetadata)?;

		<CommunityMetadata<T>>::insert(cid, &community_metadata);

		sp_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
		sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

		info!(target: LOG, "updated community metadata for cid: {:?}", cid);
		Self::deposit_event(Event::MetadataUpdated(cid));

		Ok(().into())
	}
	pub fn do_update_demurrage(
		cid: CommunityIdentifier,
		demurrage: Demurrage,
	) -> DispatchResultWithPostInfo {
		Self::ensure_cid_exists(&cid)?;

		<pallet_encointer_balances::Pallet<T>>::set_demurrage(&cid, demurrage)
			.map_err(|_| <Error<T>>::InvalidDemurrage)?;

		info!(target: LOG, " updated demurrage for cid: {:?}", cid);
		Self::deposit_event(Event::DemurrageUpdated(cid, demurrage));

		Ok(().into())
	}

	pub fn do_update_nominal_income(
		cid: CommunityIdentifier,
		nominal_income: NominalIncomeType,
	) -> DispatchResultWithPostInfo {
		Self::ensure_cid_exists(&cid)?;

		<NominalIncome<T>>::insert(cid, nominal_income);

		info!(target: LOG, " updated nominal income for cid: {:?}", cid);
		Self::deposit_event(Event::NominalIncomeUpdated(cid, nominal_income));

		Ok(().into())
	}
	fn remove_location_intern(cid: CommunityIdentifier, location: Location, geo_hash: GeoHash) {
		//remove location from locations(cid,geohash)
		let mut locations = Self::locations(cid, &geo_hash);
		let mut locations_len = 0;
		if let Ok(index) = locations.binary_search(&location) {
			locations.remove(index);
			locations_len = locations.len();
			<Locations<T>>::insert(cid, &geo_hash, locations);
		}
		// if the list from above is now empty (community has no more locations in this bucket)
		// remove cid from cids_by_geohash(geohash)
		if locations_len == 0 {
			let mut cids = Self::cids_by_geohash(&geo_hash);
			if let Ok(index) = cids.binary_search(&cid) {
				cids.remove(index);
				<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids);
			}
		}
		sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());
	}

	pub fn remove_community(cid: CommunityIdentifier) {
		info!(target: LOG, "removing community {:?}", cid);
		for (geo_hash, locations) in <Locations<T>>::iter_prefix(cid) {
			for location in locations {
				Self::remove_location_intern(cid, location, geo_hash.clone());
			}
		}

		#[allow(deprecated)]
		<Locations<T>>::remove_prefix(cid, None);

		Bootstrappers::<T>::remove(cid);

		<CommunityIdentifiers<T>>::mutate(|v| v.retain(|&x| x != cid));

		<CommunityMetadata<T>>::remove(cid);

		<NominalIncome<T>>::remove(cid);

		<pallet_encointer_balances::Pallet<T>>::purge_balances(cid);

		Self::deposit_event(Event::CommunityPurged(cid));
	}

	pub fn insert_bootstrappers(
		cid: CommunityIdentifier,
		bootstrappers: BoundedVec<T::AccountId, T::MaxBootstrappers>,
	) {
		<Bootstrappers<T>>::insert(cid, &bootstrappers);
	}

	fn solar_trip_time(from: &Location, to: &Location) -> u32 {
		// FIXME: replace by fixpoint implementation within runtime.
		let d = Pallet::<T>::haversine_distance(from, to); //orthodromic distance bewteen points [m]

		// FIXME: this will not panic, but make sure!
		let dt = (from.lon - to.lon) * 240; //time, the sun-high needs to travel between locations [s]
		let tflight = d / Self::max_speed_mps(); // time required to travel between locations at MaxSpeedMps [s]
		let dt: u32 = i64::lossy_from(dt.abs()).saturated_into();
		tflight.saturating_sub(dt)
	}

	fn ensure_cid_exists(cid: &CommunityIdentifier) -> DispatchResult {
		match Self::community_identifiers().contains(cid) {
			true => Ok(()),
			false => Err(<Error<T>>::CommunityInexistent)?,
		}
	}

	pub fn is_valid_location(loc: &Location) -> bool {
		(loc.lat < MAX_ABS_LATITUDE) &
			(loc.lat > -MAX_ABS_LATITUDE) &
			(loc.lon < DATELINE_LON) &
			(loc.lon > -DATELINE_LON)
	}

	pub fn haversine_distance(a: &Location, b: &Location) -> u32 {
		type D = Degree;
		let two = D::from_num(2);
		let theta1 = a.lat * D::lossy_from(RADIANS_PER_DEGREE);
		let theta2 = b.lat * D::lossy_from(RADIANS_PER_DEGREE);
		let delta_theta = theta1 - theta2;
		let delta_lambda = (a.lon - b.lon) * D::lossy_from(RADIANS_PER_DEGREE);

		let tmp0 = sin(delta_theta / two);
		let tmp1 = powi::<D, D>(tmp0, 2).unwrap_or_default();
		let tmp2 = cos(theta1) * cos(theta2);
		let tmp3 = sin(delta_lambda / two);
		let tmp4 = powi::<D, D>(tmp3, 2).unwrap_or_default();

		let aa = tmp1 + tmp2 * tmp4;
		let c: D = two * asin(sqrt::<D, D>(aa).unwrap_or_default());
		let d = D::from(MEAN_EARTH_RADIUS) * c;
		i64::lossy_from(d).saturated_into()
	}

	fn validate_bootstrappers(bootstrappers: &[T::AccountId]) -> DispatchResult {
		ensure!(
			bootstrappers.len() <= T::MaxBootstrappers::get() as usize,
			<Error<T>>::InvalidAmountBootstrappers
		);
		ensure!(!bootstrappers.len() >= 3, <Error<T>>::InvalidAmountBootstrappers);
		Ok(())
	}

	fn get_relevant_neighbor_buckets(
		geo_hash: &GeoHash,
		location: &Location,
	) -> Result<Vec<GeoHash>, Error<T>> {
		let mut relevant_neighbor_buckets: Vec<GeoHash> = Vec::new();
		let neighbors = geo_hash.neighbors().map_err(|_| <Error<T>>::InvalidGeohash)?;
		let (bucket_center_lon, bucket_center_lat, bucket_lon_error, bucket_lat_error) =
			geo_hash.try_as_coordinates().map_err(|_| <Error<T>>::InvalidGeohash)?;
		let bucket_min_lat = bucket_center_lat - bucket_lat_error;
		let bucket_max_lat = bucket_center_lat + bucket_lat_error;
		let bucket_min_lon = bucket_center_lon - bucket_lon_error;
		let bucket_max_lon = bucket_center_lon + bucket_lon_error;

		//check if northern neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: location.lon, lat: bucket_max_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.n)
		}

		//check if southern neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: location.lon, lat: bucket_min_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.s)
		}

		// if we assume MIN_SOLAR_TIME = 1 second and maximum latitude = 78 degrees
		// and maximum human speed 83 m/s and a BUCKET_RESOLUTION of 5
		// it is save to only consider the direct neighbours of the current bucket,
		// because it takes more more than 1 second solar time to traverse 1 bucket.

		// solar time = time human - time sun
		// width of one bucket at latitude x = 4900 * cos(x) m
		// speed human: 83 m/s
		// speed sun: (cos(x) * 111319) / 240 = (meters/degree)/(seconds/degree) = m/s
		// time sun = (4900 * cos(x)) / ((cos(x) * 111319) / 240)
		// time human = (4900 * cos(x)) / 83
		// solar time = ((4900 * cos(x)) / 83) - ((4900 * cos(x)) / ((cos(x) * 111319) / 240))
		// solve ((4900 * cos(x)) / 83) - ((4900 * cos(x)) / ((cos(x) * 111319) / 240)) = 1
		// x = 78.7036, so below 78.7036 it takes a human more than 1 second solar time to
		// traverse 1 bucket horizontally

		//check if north eastern neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_max_lon, lat: bucket_max_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.ne)
		}

		//check if eastern neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_max_lon, lat: location.lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.e)
		}

		//check if south eastern neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_max_lon, lat: bucket_min_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.se)
		}

		//check if north western neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_min_lon, lat: bucket_max_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.nw)
		}

		//check if western neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_min_lon, lat: location.lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.w)
		}

		//check if south western neighbour bucket needs to be included
		if Self::solar_trip_time(&Location { lon: bucket_min_lon, lat: bucket_min_lat }, location) <
			Self::min_solar_trip_time_s()
		{
			relevant_neighbor_buckets.push(neighbors.sw)
		}

		Ok(relevant_neighbor_buckets)
	}
	fn get_nearby_locations(location: &Location) -> Result<Vec<Location>, Error<T>> {
		let mut result: Vec<Location> = Vec::new();
		let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
			.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
		let mut relevant_buckets = Self::get_relevant_neighbor_buckets(&geo_hash, location)?;
		relevant_buckets.push(geo_hash);

		for bucket in relevant_buckets {
			for cid in Self::cids_by_geohash(&bucket) {
				result.append(&mut Self::locations(cid, &bucket).to_vec());
			}
		}
		Ok(result)
	}

	fn validate_location(location: &Location) -> DispatchResult {
		ensure!(Self::is_valid_location(location), <Error<T>>::InvalidLocation);

		// prohibit proximity to dateline
		let dateline_proxy = Location { lat: location.lat, lon: DATELINE_LON };
		if Self::haversine_distance(location, &dateline_proxy) < DATELINE_DISTANCE_M {
			warn!(target: LOG, "location too close to dateline: {:?}", location);
			return Err(<Error<T>>::MinimumDistanceViolationToDateLine)?;
		}

		let nearby_locations = Self::get_nearby_locations(location)?;
		for nearby_location in nearby_locations {
			ensure!(
				Self::solar_trip_time(location, &nearby_location) >= Self::min_solar_trip_time_s(),
				<Error<T>>::MinimumDistanceViolationToOtherLocation
			);
		}
		Ok(())
	}

	// The methods below are for the runtime api

	pub fn get_cids() -> Vec<CommunityIdentifier> {
		Self::community_identifiers().to_vec()
	}

	pub fn get_name(cid: &CommunityIdentifier) -> Option<PalletString> {
		Self::ensure_cid_exists(cid).ok()?;
		Some(Self::community_metadata(cid).name)
	}

	pub fn get_locations(cid: &CommunityIdentifier) -> Vec<Location> {
		<Locations<T>>::iter_prefix_values(cid)
			.map(|a| a.to_vec())
			.reduce(|a, b| a.iter().cloned().chain(b.iter().cloned()).collect())
			.unwrap()
	}

	pub fn get_all_balances(
		account: &T::AccountId,
	) -> Vec<(CommunityIdentifier, BalanceEntry<BlockNumberFor<T>>)> {
		let mut balances: Vec<(CommunityIdentifier, BalanceEntry<BlockNumberFor<T>>)> = vec![];
		for cid in Self::community_identifiers().into_iter() {
			if pallet_encointer_balances::Balance::<T>::contains_key(cid, account.clone()) {
				balances.push((
					cid,
					<pallet_encointer_balances::Pallet<T>>::balance_entry(cid, account.clone()),
				));
			}
		}
		balances
	}
}

mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod migrations;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
