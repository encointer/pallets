use frame_support::weights::Weight;

pub trait WeightInfo {
	fn register_bandersnatch_key() -> Weight;
	fn initiate_rings() -> Weight;
	fn continue_ring_computation_collect(n: u32) -> Weight;
	fn continue_ring_computation_build(n: u32) -> Weight;
}

impl WeightInfo for () {
	fn register_bandersnatch_key() -> Weight {
		Weight::from_parts(10_000_000, 0)
	}
	fn initiate_rings() -> Weight {
		Weight::from_parts(15_000_000, 0)
	}
	fn continue_ring_computation_collect(n: u32) -> Weight {
		Weight::from_parts(10_000_000 + 50_000 * n as u64, 0)
	}
	fn continue_ring_computation_build(n: u32) -> Weight {
		Weight::from_parts(10_000_000 + 100_000 * n as u64, 0)
	}
}
