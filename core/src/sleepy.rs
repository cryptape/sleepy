use util::error::*;
use util::Hashable;
use std::result;
use chain::block::Block;
use chain::transaction::SignedTransaction;
use error::*;
use util::config::SleepyConfig;
use util::U256;
use std::sync::Arc;
use parking_lot::RwLock;

pub struct Sleepy {
    /// sleepy config
    config: Arc<RwLock<SleepyConfig>>
}

impl Sleepy {
    pub fn new(config: Arc<RwLock<SleepyConfig>>) -> Self {
        Sleepy { config: config.clone() }
    }

    pub fn verify_tx_basic(&self, stx: &SignedTransaction) -> result::Result<(), Error> {
        stx.recover_public()?;
        Ok(())
    }

    pub fn verify_block_basic(&self, block: &Block) -> result::Result<(), Error> {
        // if !sigblk.verify() {
        //     info!("block signature verify fail");
        //     return Err(Error::InvalidSignature(sigblk.signature));
        // }
        let proof_pub = block.proof_public()?;
        let sign_pub = block.sign_public()?;
        let config = self.config.read();
        if !config.check_keys(&proof_pub, &sign_pub) {
            return Err(Error::InvalidPublicKey(proof_pub, sign_pub));
        }

        let block_difficulty: U256 = block.proof.time_signature.sha3().into();
        if block_difficulty > config.get_difficulty() {
            return Err(Error::InvalidProofOfWork(OutOfBounds {
                                                     min: None,
                                                     max: Some(config.get_difficulty()),
                                                     found: block_difficulty,
                                                 }));
        }

        let max = config.timestamp_now() + 2 * config.hz * config.duration;

        if max < block.timestamp {
            return Err(Error::BlockInFuture(OutOfBounds {
                                                min: None,
                                                max: Some(max),
                                                found: block.timestamp,
                                            }));
        }

        Ok(())
    }
}