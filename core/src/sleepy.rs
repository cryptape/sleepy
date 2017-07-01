use util::type::*;
use util::error::*;
use util::config::SleepyConfig;
use bigint::hash::{H256, H520};
use bigint::uint::U256;

pub struct Sleepy {
    /// sleepy config
    config: Arc<RwLock<SleepyConfig>>,
}

impl Sleepy {
    pub fn new(config: Arc<RwLock<SleepyConfig>>) -> Self {
		Arc::new(Sleepy {
			config: config.clone(),
		})
	}

    fn verify_block_basic(&self, sigblk: &SignedBlock) -> result::Result<(), Error> {
        let block = sigblk.block;
        let 
        
    }

}