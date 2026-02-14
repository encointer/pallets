use crate as dut;
use encointer_primitives::{balances::BalanceType, scheduler::CeremonyPhaseType};
use sp_runtime::BuildStorage;
use test_utils::*;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;

frame_support::construct_runtime!(
	pub enum TestRuntime
	{
		System: frame_system::{Pallet, Call, Config<T>, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		EncointerScheduler: pallet_encointer_scheduler::{Pallet, Call, Storage, Config<T>, Event},
		EncointerCommunities: pallet_encointer_communities::{Pallet, Call, Storage, Event<T>},
		EncointerCeremonies: pallet_encointer_ceremonies::{Pallet, Call, Storage, Event<T>},
		EncointerBalances: pallet_encointer_balances::{Pallet, Call, Storage, Event<T>},
		EncointerReputationRing: dut::{Pallet, Call, Storage, Event<T>},
	}
);

impl dut::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type MaxRingSize = ConstU32<2048>;
	type ChunkSize = ConstU32<100>;
}

// boilerplate
impl_frame_system!(TestRuntime);
impl_timestamp!(TestRuntime, EncointerScheduler);
impl_balances!(TestRuntime, System);
impl_encointer_balances!(TestRuntime);
impl_encointer_communities!(TestRuntime);
impl_encointer_scheduler!(TestRuntime);
impl_encointer_ceremonies!(TestRuntime);

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::<TestRuntime>::default().build_storage().unwrap();

	pallet_encointer_scheduler::GenesisConfig::<TestRuntime> {
		current_ceremony_index: 7,
		phase_durations: vec![
			(CeremonyPhaseType::Registering, ONE_DAY),
			(CeremonyPhaseType::Assigning, ONE_DAY),
			(CeremonyPhaseType::Attesting, ONE_DAY),
		],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_encointer_ceremonies::GenesisConfig::<TestRuntime> {
		ceremony_reward: BalanceType::from_num(1),
		location_tolerance: LOCATION_TOLERANCE,
		time_tolerance: TIME_TOLERANCE,
		inactivity_timeout: 12,
		endorsement_tickets_per_bootstrapper: 50,
		endorsement_tickets_per_reputable: 2,
		reputation_lifetime: 5,
		meetup_time_offset: 0,
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	pallet_encointer_communities::GenesisConfig::<TestRuntime> {
		min_solar_trip_time_s: 1,
		max_speed_mps: 83,
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();

	t.into()
}
