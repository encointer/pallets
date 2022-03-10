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
//!

#![cfg_attr(not(feature = "std"), no_std)]

use codec::Encode;
use encointer_primitives::{
	balances::{BalanceEntry, BalanceType, Demurrage},
	common::PalletString,
	communities::{
		consts::*, validate_demurrage, validate_nominal_income, CommunityIdentifier,
		CommunityMetadata as CommunityMetadataType, Degree, GeoHash, Location, LossyFrom,
		NominalIncome as NominalIncomeType,
	},
	fixed::transcendental::{asin, cos, powi, sin, sqrt},
	scheduler::CeremonyPhaseType,
};
use frame_support::ensure;
use log::{info, warn};
use sp_runtime::{DispatchResult, SaturatedConversion};
use sp_std::{prelude::*, result::Result};

// Logger target
const LOG: &str = "encointer";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use encointer_primitives::communities::{MaxSpeedMpsType, MinSolarTripTimeType};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub (super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config:
		frame_system::Config + encointer_scheduler::Config + encointer_balances::Config
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Required origin for adding or updating a community (though can always be Root).
		type CommunityMaster: EnsureOrigin<Self::Origin>;
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Add a new community.
		///
		/// May only be called from `T::CommunityMaster`.
		#[pallet::weight(10_000)]
		pub fn new_community(
			origin: OriginFor<T>,
			location: Location,
			bootstrappers: Vec<T::AccountId>,
			community_metadata: CommunityMetadataType,
			demurrage: Option<Demurrage>,
			nominal_income: Option<NominalIncomeType>,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			Self::validate_bootstrappers(&bootstrappers)?;
			community_metadata
				.validate()
				.map_err(|_| <Error<T>>::InvalidCommunityMetadata)?;
			if let Some(d) = demurrage {
				validate_demurrage(&d).map_err(|_| <Error<T>>::InvalidDemurrage)?;
			}
			if let Some(i) = nominal_income {
				validate_nominal_income(&i).map_err(|_| <Error<T>>::InvalidNominalIncome)?;
			}

			let cid = CommunityIdentifier::new(location, bootstrappers.clone())
				.map_err(|_| Error::<T>::InvalidLocation)?;
			let cids = Self::community_identifiers();
			ensure!(!cids.contains(&cid), Error::<T>::CommunityAlreadyRegistered);

			Self::validate_location(&location)?;
			// All checks done, now mutate state
			let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
				.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
			let mut locations: Vec<Location> = Vec::new();

			// insert cid into cids_by_geohash map
			let mut cids_in_bucket = Self::cids_by_geohash(&geo_hash);
			match cids_in_bucket.binary_search(&cid) {
				Ok(_) => (),
				Err(index) => {
					cids_in_bucket.insert(index, cid.clone());
					<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids_in_bucket);
				},
			}

			// insert location into cid -> geohash -> location map
			locations.push(location);
			<Locations<T>>::insert(&cid, geo_hash, locations);

			<CommunityIdentifiers<T>>::mutate(|v| v.push(cid));

			<Bootstrappers<T>>::insert(&cid, &bootstrappers);
			<CommunityMetadata<T>>::insert(&cid, &community_metadata);

			demurrage.map(|d| <encointer_balances::Pallet<T>>::set_demurrage(&cid, d));
			nominal_income.map(|i| <NominalIncome<T>>::insert(&cid, i));

			sp_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
			sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

			Self::deposit_event(Event::CommunityRegistered(cid));
			info!(target: LOG, "registered community with cid: {:?}", cid);
			Ok(().into())
		}

		/// Add a new meetup `location` to the community with `cid`.
		///
		/// May only be called from `T::CommunityMaster`.
		///
		/// Todo: Replace `T::CommunityMaster` with community governance: #137.
		#[pallet::weight(10_000)]
		pub fn add_location(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			location: Location,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;

			ensure!(
				<encointer_scheduler::Pallet<T>>::current_phase() == CeremonyPhaseType::REGISTERING,
				Error::<T>::RegistrationPhaseRequired
			);
			Self::ensure_cid_exists(&cid)?;
			Self::validate_location(&location)?;
			let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
				.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
			// insert location into locations
			let mut locations = Self::locations(&cid, &geo_hash);
			match locations.binary_search(&location) {
				Ok(_) => (),
				Err(index) => {
					locations.insert(index, location);
					<Locations<T>>::insert(&cid, &geo_hash, locations);
				},
			}
			// check if cid is in cids_by_geohash, if not, add it
			let mut cids = Self::cids_by_geohash(&geo_hash);
			match cids.binary_search(&cid) {
				Ok(_) => (),
				Err(index) => {
					cids.insert(index, cid.clone());
					<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids);
				},
			}
			sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

			info!(target: LOG, "added location {:?} to community with cid: {:?}", location, cid);
			Self::deposit_event(Event::LocationAdded(cid, location));
			Ok(().into())
		}

		/// Remove an existing meetup `location` from the community with `cid`.
		///
		/// May only be called from `T::CommunityMaster`.
		///
		/// Todo: Replace `T::CommunityMaster` with community governance: #137.
		#[pallet::weight(10_000)]
		pub fn remove_location(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			location: Location,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;

			ensure!(
				<encointer_scheduler::Pallet<T>>::current_phase() == CeremonyPhaseType::REGISTERING,
				Error::<T>::RegistrationPhaseRequired
			);
			Self::ensure_cid_exists(&cid)?;

			let geo_hash = GeoHash::try_from_params(location.lat, location.lon)
				.map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
			Self::remove_location_intern(cid, location, geo_hash);
			info!(target: LOG, "removed location {:?} to community with cid: {:?}", location, cid);
			Self::deposit_event(Event::LocationRemoved(cid, location));
			Ok(().into())
		}

		/// Update the metadata of the community with `cid`.
		///
		/// May only be called from `T::CommunityMaster`.
		#[pallet::weight(10_000)]
		pub fn update_community_metadata(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			community_metadata: CommunityMetadataType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;

			Self::ensure_cid_exists(&cid)?;
			community_metadata
				.validate()
				.map_err(|_| <Error<T>>::InvalidCommunityMetadata)?;

			<CommunityMetadata<T>>::insert(&cid, &community_metadata);

			sp_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
			sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

			info!(target: LOG, "updated community metadata for cid: {:?}", cid);
			Self::deposit_event(Event::MetadataUpdated(cid));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn update_demurrage(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			demurrage: BalanceType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;

			Self::ensure_cid_exists(&cid)?;
			validate_demurrage(&demurrage).map_err(|_| <Error<T>>::InvalidDemurrage)?;
			Self::ensure_cid_exists(&cid)?;

			<encointer_balances::Pallet<T>>::set_demurrage(&cid, demurrage);

			info!(target: LOG, " updated demurrage for cid: {:?}", cid);
			Self::deposit_event(Event::DemurrageUpdated(cid, demurrage));

			Ok(().into())
		}

		#[pallet::weight(10_000)]
		pub fn update_nominal_income(
			origin: OriginFor<T>,
			cid: CommunityIdentifier,
			nominal_income: NominalIncomeType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;

			Self::ensure_cid_exists(&cid)?;
			validate_nominal_income(&nominal_income)
				.map_err(|_| <Error<T>>::InvalidNominalIncome)?;
			Self::ensure_cid_exists(&cid)?;

			<NominalIncome<T>>::insert(&cid, &nominal_income);

			info!(target: LOG, " updated nominal income for cid: {:?}", cid);
			Self::deposit_event(Event::NominalIncomeUpdated(cid, nominal_income));

			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_min_solar_trip_time_s(
			origin: OriginFor<T>,
			min_solar_trip_time_s: MinSolarTripTimeType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			<MinSolarTripTimeS<T>>::put(min_solar_trip_time_s);
			Ok(().into())
		}

		#[pallet::weight((1000, DispatchClass::Operational,))]
		pub fn set_max_speed_mps(
			origin: OriginFor<T>,
			max_speed_mps: MaxSpeedMpsType,
		) -> DispatchResultWithPostInfo {
			T::CommunityMaster::ensure_origin(origin)?;
			<MaxSpeedMps<T>>::put(max_speed_mps);
			Ok(().into())
		}

		#[pallet::weight(10_000)]
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
	}

	#[pallet::storage]
	#[pallet::getter(fn cids_by_geohash)]
	pub(super) type CommunityIdentifiersByGeohash<T: Config> =
		StorageMap<_, Identity, GeoHash, Vec<CommunityIdentifier>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn locations)]
	pub(super) type Locations<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		CommunityIdentifier,
		Identity,
		GeoHash,
		Vec<Location>,
		ValueQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn bootstrappers)]
	pub(super) type Bootstrappers<T: Config> =
		StorageMap<_, Blake2_128Concat, CommunityIdentifier, Vec<T::AccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn community_identifiers)]
	pub(super) type CommunityIdentifiers<T: Config> =
		StorageValue<_, Vec<CommunityIdentifier>, ValueQuery>;

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

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub min_solar_trip_time_s: MinSolarTripTimeType,
		pub max_speed_mps: MaxSpeedMpsType,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self { min_solar_trip_time_s: Default::default(), max_speed_mps: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			<MinSolarTripTimeS<T>>::put(&self.min_solar_trip_time_s);
			<MaxSpeedMps<T>>::put(&self.max_speed_mps);
		}
	}
}

impl<T: Config> Pallet<T> {
	fn remove_location_intern(cid: CommunityIdentifier, location: Location, geo_hash: GeoHash) {
		//remove location from locations(cid,geohash)
		let mut locations = Self::locations(&cid, &geo_hash);
		let mut locations_len = 0;
		match locations.binary_search(&location) {
			Ok(index) => {
				locations.remove(index);
				locations_len = locations.len();
				<Locations<T>>::insert(&cid, &geo_hash, locations);
			},
			Err(_) => (),
		}
		// if the list from above is now empty (community has no more locations in this bucket)
		// remove cid from cids_by_geohash(geohash)
		if locations_len == 0 {
			let mut cids = Self::cids_by_geohash(&geo_hash);
			match cids.binary_search(&cid) {
				Ok(index) => {
					cids.remove(index);
					<CommunityIdentifiersByGeohash<T>>::insert(&geo_hash, cids);
				},
				Err(_) => (),
			}
		}
		sp_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());
	}

	pub fn remove_community(cid: CommunityIdentifier) {
		for (geo_hash, locations) in <Locations<T>>::iter_prefix(&cid) {
			for location in locations {
				Self::remove_location_intern(cid, location, geo_hash.clone());
			}
		}

		<Locations<T>>::remove_prefix(cid, None);

		Bootstrappers::<T>::remove(cid);

		<CommunityIdentifiers<T>>::mutate(|v| v.retain(|&x| x != cid));

		<CommunityMetadata<T>>::remove(cid);

		<NominalIncome<T>>::remove(cid);

		<encointer_balances::Pallet<T>>::purge_balances(cid);
	}

	pub fn insert_bootstrappers(cid: CommunityIdentifier, bootstrappers: Vec<T::AccountId>) {
		<Bootstrappers<T>>::insert(&cid, &bootstrappers);
	}

	fn solar_trip_time(from: &Location, to: &Location) -> u32 {
		// FIXME: replace by fixpoint implementation within runtime.
		let d = Pallet::<T>::haversine_distance(&from, &to); //orthodromic distance bewteen points [m]

		// FIXME: this will not panic, but make sure!
		let dt = (from.lon - to.lon) * 240; //time, the sun-high needs to travel between locations [s]
		let tflight = d / Self::max_speed_mps(); // time required to travel between locations at MaxSpeedMps [s]
		let dt: u32 = i64::lossy_from(dt.abs()).saturated_into();
		tflight.checked_sub(dt).unwrap_or(0)
	}

	fn ensure_cid_exists(cid: &CommunityIdentifier) -> DispatchResult {
		match Self::community_identifiers().contains(&cid) {
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

	fn validate_bootstrappers(bootstrappers: &Vec<T::AccountId>) -> DispatchResult {
		ensure!(bootstrappers.len() <= 1000, <Error<T>>::InvalidAmountBootstrappers);
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
				result.append(&mut Self::locations(&cid, &bucket).clone());
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
			return Err(<Error<T>>::MinimumDistanceViolationToDateLine)?
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
		Self::community_identifiers()
	}

	pub fn get_name(cid: &CommunityIdentifier) -> Option<PalletString> {
		Self::ensure_cid_exists(cid).ok()?;
		Some(Self::community_metadata(cid).name)
	}

	pub fn get_locations(cid: &CommunityIdentifier) -> Vec<Location> {
		<Locations<T>>::iter_prefix_values(&cid)
			.reduce(|a, b| a.iter().cloned().chain(b.iter().cloned()).collect())
			.unwrap()
	}

	pub fn get_all_balances(
		account: &T::AccountId,
	) -> Vec<(CommunityIdentifier, BalanceEntry<T::BlockNumber>)> {
		let mut balances: Vec<(CommunityIdentifier, BalanceEntry<T::BlockNumber>)> = vec![];
		for cid in Self::community_identifiers().into_iter() {
			if encointer_balances::Balance::<T>::contains_key(cid, account.clone()) {
				balances.push((
					cid,
					<encointer_balances::Pallet<T>>::balance_entry(cid, &account.clone()),
				));
			}
		}
		return balances
	}
}

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
