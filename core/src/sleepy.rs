use util::error::*;
use util::Hashable;
use std::result;
use chain::{SignedBlock, SignedTransaction};
use error::*;
use util::config::SleepyConfig;
use bigint::uint::U256;
use std::sync::Arc;
use parking_lot::RwLock;
use timesync::{TimeSyncer};

pub struct Sleepy {
    /// sleepy config
    config: Arc<RwLock<SleepyConfig>>,
    time_syncer: Arc<RwLock<TimeSyncer>>,
}

impl Sleepy {
    pub fn new(config: Arc<RwLock<SleepyConfig>>, time_syncer: Arc<RwLock<TimeSyncer>>) -> Self {
        Sleepy { config: config.clone(), time_syncer: time_syncer.clone() }
    }

    pub fn verify_tx_basic(&self, stx: &SignedTransaction) -> result::Result<(), Error> {
        if !stx.verify(&stx.signer) {
            warn!("Signed transaction verify fail");
            return Err(Error::InvalidSignature(stx.signature));
        }

        let config = self.config.read();
        if !config.check_keys(&stx.signer) {
            return Err(Error::InvalidPublicKey(stx.signer));
        }

        Ok(())
    }

    pub fn verify_block_basic(&self, sigblk: &SignedBlock) -> result::Result<(), Error> {
        // if !sigblk.verify() {
        //     info!("block signature verify fail");
        //     return Err(Error::InvalidSignature(sigblk.signature));
        // }
        let block = &sigblk.block;
        let minerkey = block.proof.key;
        let config = self.config.read();
        if !block.proof.verify() {
            warn!("block proof verify fail");
            return Err(Error::InvalidSignature(block.proof.signature));
        }

        if !config.check_keys(&minerkey) {
            return Err(Error::InvalidPublicKey(minerkey));
        }

        let block_difficulty: U256 = block.proof.signature.sha3().into();
        if block_difficulty > config.get_difficulty() {
            return Err(Error::InvalidProofOfWork(OutOfBounds {
                                                     min: None,
                                                     max: Some(config.get_difficulty()),
                                                     found: block_difficulty,
                                                 }));
        }

        if ({self.time_syncer.read().time_now_ms()} * config.hz / 1000 + 2 * config.hz * config.duration) < block.proof.timestamp {
            return Err(Error::BlockInFuture(OutOfBounds {
                                                min: None,
                                                max: Some({self.time_syncer.read().time_now_ms()} * config.hz / 1000 + 2 * config.hz * config.duration),
                                                found: block.proof.timestamp,
                                            }));
        }

        Ok(())
    }
}