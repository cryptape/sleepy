use util::type::*;
use util::error::*;
use util::config::SleepyConfig;
use bigint::hash::{H256, H520};
use bigint::uint::U256;

/// Sleepy params.
#[derive(Debug, PartialEq)]
pub struct SleepyParams {
    /// difficulty.
	pub difficulty: U256,
    /// time interval
    pub interval: u32,
    /// pki
    pub config: SleepyConfig, 
}