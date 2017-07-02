use util::error::*;
use util::Hashable;
use std::result;
use chain::{SignedBlock};
use error::*;
use util::config::SleepyConfig;
use bigint::uint::U256;
use std::sync::Arc;
use parking_lot::RwLock;

pub struct Sleepy {
    /// sleepy config
    config: Arc<RwLock<SleepyConfig>>,
}

impl Sleepy {
    pub fn new(config: Arc<RwLock<SleepyConfig>>) -> Self {
	    Sleepy {
			config: config.clone(),
		}
	}

    pub fn verify_block_basic(&self, sigblk: &SignedBlock) -> result::Result<(), Error> {
        if !sigblk.verify(){
            return Err(Error::InvalidSignature(sigblk.signature));
        }
        let block = &sigblk.block;
        let singerkey = sigblk.singer;
        let minerkey = block.proof.key;
        let config = self.config.read();
        if !block.proof.verify() {
            return Err(Error::InvalidSignature(block.proof.signature));
        }

        if !config.check_keys(minerkey, singerkey) {
            return Err(Error::InvalidPublicKey((minerkey, singerkey)));
        }
        let block_difficulty : U256 = block.proof.signature.sha3().into();
        if  block_difficulty > config.get_difficulty() {
            return Err(Error::InvalidProofOfWork(OutOfBounds { min: None, max: Some(config.get_difficulty()), found: block_difficulty }));
        }
        
        if config.timestamp_now() < block.proof.timestamp {
            return Err(Error::BlockInFuture(OutOfBounds { min: None, max: Some(config.timestamp_now()), found: block.proof.timestamp }));
        }

        Ok(())

        
    }

}