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
use log::{info, warn};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage, ensure,
    storage::{StorageMap, StorageValue},
};
use frame_system::{ensure_root, ensure_signed};
use rstd::prelude::*;
use rstd::result::Result;
use fixed::transcendental::{asin, cos, powi, sin, sqrt};
use runtime_io::hashing::blake2_256;
use sp_runtime::{DispatchResult, SaturatedConversion, DispatchResultWithInfo};

use geohash::{neighbor, encode, decode, Direction, GeohashError};

use encointer_primitives::{
    balances::{BalanceType, Demurrage},
    common::PalletString,
    communities::{
        consts::*, validate_demurrage, validate_nominal_income, CommunityIdentifier,
        CommunityMetadata as CommunityMetadataType, Degree, Location, LossyFrom,
        NominalIncome as NominalIncomeType,
    },
};

pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
}

// Logger target
const LOG: &str = "encointer";

decl_storage! {
    trait Store for Module<T: Config> as EncointerCommunities {
        Locations get(fn locations): map hasher(blake2_128_concat) CommunityIdentifier => Vec<Location>;
        Bootstrappers get(fn bootstrappers): map hasher(blake2_128_concat) CommunityIdentifier => Vec<T::AccountId>;
        CommunityIdentifiers get(fn community_identifiers): Vec<CommunityIdentifier>;
        CommunityMetadata get(fn community_metadata): map hasher(blake2_128_concat) CommunityIdentifier => CommunityMetadataType;
        pub DemurragePerBlock get(fn demurrage_per_block): map hasher(blake2_128_concat) CommunityIdentifier => Demurrage;
        /// Amount of UBI to be paid for every attended ceremony.
        pub NominalIncome get(fn nominal_income): map hasher(blake2_128_concat) CommunityIdentifier => NominalIncomeType;
        // TODO: replace this with on-chain governance
        CommunityMaster get(fn community_master) config(): T::AccountId;
    }
}

decl_module! {
    pub struct Module<T: Config> for enum Call where origin: T::Origin {
        fn deposit_event() = default;
        type Error = Error<T>;
        // FIXME: this function has complexity O(n^2)!
        // where n is the number of all locations of all communities
        // this should be run off-chain in substraTEE-worker later
        #[weight = 10_000]
        pub fn new_community(
            origin,
            loc: Vec<Location>,
            bootstrappers: Vec<T::AccountId>,
            community_metadata: CommunityMetadataType,
            demurrage: Option<Demurrage>,
            nominal_income: Option<NominalIncomeType>
        ) {
            let sender = ensure_signed(origin)?;
            Self::validate_bootstrappers(&bootstrappers)?;
            community_metadata.validate().map_err(|_|  <Error<T>>::InvalidCommunityMetadata)?;
            if let Some(d) = demurrage {
                validate_demurrage(&d).map_err(|_| <Error<T>>::InvalidDemurrage)?;
            }
            if let Some(i) = nominal_income {
                validate_nominal_income(&i).map_err(|_| <Error<T>>::InvalidNominalIncome)?;
            }

            let cid = CommunityIdentifier::from(blake2_256(&(loc.clone(), bootstrappers.clone()).encode()));
            let cids = Self::community_identifiers();
            ensure!(!cids.contains(&cid), "community already registered");
            Self::validate_locations(&loc)?;

            // All checks done, now mutate state

            <CommunityIdentifiers>::mutate(|v| v.push(cid));
            <Locations>::insert(&cid, &loc);
            <Bootstrappers<T>>::insert(&cid, &bootstrappers);
            <CommunityMetadata>::insert(&cid, &community_metadata);

            demurrage.map(|d| <DemurragePerBlock>::insert(&cid, d));
            nominal_income.map(|i| <NominalIncome>::insert(&cid, i));

            runtime_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
            runtime_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

            Self::deposit_event(RawEvent::CommunityRegistered(sender, cid));
            info!(target: LOG, "registered community with cid: {:?}", cid);
        }

        #[weight = 10_000]
        fn update_community_medadata(origin, cid: CommunityIdentifier, community_metadata: CommunityMetadataType) {
            ensure_root(origin)?;
            Self::ensure_cid_exists(&cid)?;
            community_metadata.validate().map_err(|_|  <Error<T>>::InvalidCommunityMetadata)?;

            <CommunityMetadata>::insert(&cid, &community_metadata);

            runtime_io::offchain_index::set(&cid.encode(), &community_metadata.name.encode());
            runtime_io::offchain_index::set(CACHE_DIRTY_KEY, &true.encode());

            Self::deposit_event(RawEvent::MetadataUpdated(cid));
            info!(target: LOG, "updated community metadata for cid: {:?}", cid);
        }

        #[weight = 10_000]
        fn update_demurrage(origin, cid: CommunityIdentifier, demurrage: BalanceType) {
            ensure_root(origin)?;
            validate_demurrage(&demurrage).map_err(|_| <Error<T>>::InvalidDemurrage)?;
            Self::ensure_cid_exists(&cid)?;

            <DemurragePerBlock>::insert(&cid, &demurrage);
            Self::deposit_event(RawEvent::DemurrageUpdated(cid, demurrage));
            info!(target: LOG, " updated demurrage for cid: {:?}", cid);
        }

        #[weight = 10_000]
        fn update_nominal_income(origin, cid: CommunityIdentifier, nominal_income: NominalIncomeType) {
            ensure_root(origin)?;
            validate_nominal_income(&nominal_income).map_err(|_| <Error<T>>::InvalidNominalIncome)?;
            Self::ensure_cid_exists(&cid)?;

            <NominalIncome>::insert(&cid, &nominal_income);
            Self::deposit_event(RawEvent::NominalIncomeUpdated(cid, nominal_income));
            info!(target: LOG, " updated nominal income for cid: {:?}", cid);
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Config>::AccountId,
    {
        /// A new community was registered \[who, community_identifier\]
        CommunityRegistered(AccountId, CommunityIdentifier),
        /// CommunityMetadata was updated \[community_identifier\]
        MetadataUpdated(CommunityIdentifier),
        /// A community's nominal income was updated \[community_identifier, new_income\]
        NominalIncomeUpdated(CommunityIdentifier, NominalIncomeType),
        /// A community's demurrage was updated \[community_identifier, new_demurrage\]
        DemurrageUpdated(CommunityIdentifier, Demurrage),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        /// Too many locations supplied
        TooManyLocations,
        /// Too few locations supplied
        TooFewLocations,
        /// Location is not a valid geolocation
        InvalidLocation,
        /// Invalid amount of bootstrappers supplied. Needs to be \[3, 12\]
        InvalidAmountBootstrappers,
        /// minimum distance violation to other location
        MinimumDistanceViolationToOtherLocation,
        /// minimum distance violated towards pole
        MinimumDistanceViolationToPole,
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
        InvalidLocationForGeohash
    }
}

impl<T: Config> Module<T> {
    fn solar_trip_time(from: &Location, to: &Location) -> i32 {
        // FIXME: replace by fixpoint implementation within runtime.
        let d = Module::<T>::haversine_distance(&from, &to) as i32;
        // FIXME: this will not panic, but make sure!
        let dt = from
            .lon
            .checked_sub(to.lon)
            .unwrap()
            .checked_div(Degree::from_num(1))
            .unwrap()
            .checked_mul(Degree::from_num(240))
            .unwrap(); // 24h * 3600s / 360° = 240s/°
        let tflight = d.checked_div(MAX_SPEED_MPS).unwrap();
        let dt: i32 = i64::lossy_from(dt.abs()).saturated_into();
        tflight - dt
    }

    fn ensure_cid_exists(cid: &CommunityIdentifier) -> DispatchResult {
        match Self::community_identifiers().contains(&cid) {
            true => Ok(()),
            false => Err(<Error<T>>::CommunityInexistent)?,
        }
    }

    pub fn is_valid_geolocation(loc: &Location) -> bool {
        (loc.lat < MAX_ABS_LATITUDE)
            & (loc.lat > -MAX_ABS_LATITUDE)
            & (loc.lon < DATELINE_LON)
            & (loc.lon > -DATELINE_LON)
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
        ensure!(
            bootstrappers.len() <= 1000,
            <Error<T>>::InvalidAmountBootstrappers
        );
        ensure!(
            !bootstrappers.len() >= 3,
            <Error<T>>::InvalidAmountBootstrappers
        );
        Ok(())
    }

    fn get_nearby_locations(location: &Location) -> Result<Vec<Location>, Error<T>>{
        let result : Vec<Location> = Vec::new();

        let hash = encode(location.lon, location.lat, 7usize).map_err(|_| <Error<T>>::InvalidLocationForGeohash)?;
        warn!(target: LOG, "computed geohash: {:?}", hash);

        Ok(result)
    }

    fn validate_location(location: &Location) -> DispatchResult {
        ensure!(Self::is_valid_geolocation(location), <Error<T>>::InvalidLocation);
        let nearby_locations = Self::get_nearby_locations(location)?;
        for nearby_location in nearby_locations.iter() {
            ensure!(
                    Self::haversine_distance(location, &nearby_location) >= MIN_DISTANCE_BETWEEN_LOCATIONS,
                    <Error<T>>::MinimumDistanceViolationToOtherLocation
                );
        }
        // prohibit proximity to dateline
        let dateline_proxy = Location {
            lat: location.lat,
            lon: DATELINE_LON,
        };
        if Self::haversine_distance(location, &dateline_proxy) < DATELINE_DISTANCE_M {
            warn!(target: LOG, "location too close to dateline: {:?}", location);
            return Err(<Error<T>>::MinimumDistanceViolationToDateLine)?;
        }
        Ok(())
    }

    fn validate_locations(locations: &Vec<Location>) -> DispatchResult {
        ensure!(locations.len() <= 10, <Error<T>>::TooManyLocations);
        ensure!(!locations.is_empty(), <Error<T>>::TooFewLocations);

        for location in locations.iter() {
            Self::validate_location(&location)?;
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
}

#[cfg(test)]
#[macro_use]
extern crate approx;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;