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
//! - modify community characteristics
//!

#![cfg_attr(not(feature = "std"), no_std)]

// use host_calls::runtime_interfaces;
use frame_support::{
    debug, decl_error, decl_event, decl_module, decl_storage,
    dispatch::DispatchResult,
    ensure,
    storage::{StorageMap, StorageValue},
};
use frame_system::ensure_signed;

use rstd::prelude::*;

use codec::Encode;
use fixed::transcendental::{asin, cos, powi, sin, sqrt};
use runtime_io::hashing::blake2_256;

use encointer_primitives::{
    balances::BalanceType,
    communities::{
        consts::*, CommunityIdentifier, CommunityMetadata as CommunityMetadataType, Degree,
        Demurrage, Location, LossyFrom,
    },
};
use sp_runtime::SaturatedConversion;

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
        CommunityMetadata get(fn community_properties): map hasher(blake2_128_concat) CommunityIdentifier => CommunityMetadataType;
        /// If it is empty, the genesis config's default is used.
        pub DemurragePerBlock get(fn demurrage_per_block): map hasher(blake2_128_concat) CommunityIdentifier => Demurrage;
        /// Amount of UBI to be paid for every attended ceremony.         /// If it is empty, the genesis config's default is used.
        pub NominalIncome get(fn nominal_income): map hasher(blake2_128_concat) CommunityIdentifier => BalanceType;
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
        pub fn new_community(origin,
            loc: Vec<Location>,
            bootstrappers: Vec<T::AccountId>,
            community_metadata: CommunityMetadataType,
            demurrage: Option<Demurrage>,
            nominal_income: Option<BalanceType>
        ) -> DispatchResult {
            debug::RuntimeLogger::init();
            let sender = ensure_signed(origin)?;
            let cid = CommunityIdentifier::from(blake2_256(&(loc.clone(), bootstrappers.clone()).encode()));
            let cids = Self::community_identifiers();
            ensure!(!cids.contains(&cid), "community already registered");

            for l1 in loc.iter() {
                ensure!(Self::is_valid_geolocation(&l1), "invalid geolocation specified");
                //test within this communities' set
                for l2 in loc.iter() {
                    if l2 == l1 { continue }
                    ensure!(Self::solar_trip_time(&l1, &l2) >= MIN_SOLAR_TRIP_TIME_S, "minimum solar trip time violated within supplied locations");
                }
                // prohibit proximity to poles
                if Self::haversine_distance(&l1, &NORTH_POLE) < DATELINE_DISTANCE_M
                    || Self::haversine_distance(&l1, &SOUTH_POLE) < DATELINE_DISTANCE_M {
                    debug::warn!(target: LOG, "location too close to pole: {:?}", l1);
                    return Err(<Error<T>>::MinimumDistanceViolationToPole.into());
                }
                // prohibit proximity to dateline
                let dateline_proxy = Location { lat: l1.lat, lon: DATELINE_LON };
                if Self::haversine_distance(&l1, &dateline_proxy) < DATELINE_DISTANCE_M {
                    debug::warn!(target: LOG, "location too close to dateline: {:?}", l1);
                    return Err(<Error<T>>::MinimumDistanceViolationToDateLine.into());
                }
                // test against all other communities globally
                for other in cids.iter() {
                    for l2 in Self::locations(other) {
                        if Self::solar_trip_time(&l1, &l2) < MIN_SOLAR_TRIP_TIME_S {
                            debug::warn!(target: LOG,
                                "location {:?} too close to previously registered location {:?} with cid {:?}",
                                l1, l2, other);
                            return Err(<Error<T>>::MinimumDistanceViolationToOtherCommunity.into());
                        }
                    }
                }
            }

            <CommunityIdentifiers>::mutate(|v| v.push(cid));
            <Locations>::insert(&cid, &loc);
            <Bootstrappers<T>>::insert(&cid, &bootstrappers);
            <CommunityMetadata>::insert(&cid, community_metadata);

            if demurrage.is_some() {
                <DemurragePerBlock>::insert(&cid, demurrage.unwrap());
            }
            if nominal_income.is_some() {
                <NominalIncome>::insert(&cid, nominal_income.unwrap());
            }

            Self::deposit_event(RawEvent::CommunityRegistered(sender, cid));
            debug::info!(target: LOG, "registered community with cid: {:?}", cid);
            Ok(())
        }
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Config>::AccountId,
    {
        CommunityRegistered(AccountId, CommunityIdentifier),
    }
);

decl_error! {
    pub enum Error for Module<T: Config> {
        /// minimum distance violated towards pole
        MinimumDistanceViolationToPole,
        /// minimum distance violated towards dateline
        MinimumDistanceViolationToDateLine,
        /// minimum distance violated towards other community's location
        MinimumDistanceViolationToOtherCommunity,
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

    pub fn is_valid_geolocation(loc: &Location) -> bool {
        if (loc.lat > NORTH_POLE.lat)
            | (loc.lat < SOUTH_POLE.lat)
            | (loc.lon > DATELINE_LON)
            | (loc.lon < -DATELINE_LON)
        {
            return false;
        }
        true
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
}

#[cfg(test)]
#[macro_use]
extern crate approx;

#[cfg(test)]
mod tests;
