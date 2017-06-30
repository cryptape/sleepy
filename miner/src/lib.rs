extern crate util;
extern crate time;
extern crate crypto;
extern crate bigint;
#[macro_use]
extern crate log;

use std::sync::mpsc::Sender;
use util::types::{Block, Chain};
use std::thread;
use std::time::Duration;
use crypto::{PrivKey, sign};
use bigint::hash::{H256, H520};
use bigint::uint::U256;
use util::Hashable;
use std::sync::Arc;

pub fn start_miner(tx: Sender<Block>, chain: Arc<Chain>, privkey: &PrivKey) {
    let difficulty : H256 = (U256::max_value() / U256::from(4 * 6 * 10)).into();
    let tx = tx.clone();
    let chain = chain.clone();
    let privkey = privkey.clone();
    thread::spawn(move || {
        info!("start mining!");
        loop {
            let (h, pre_hash) = chain.get_status();
            let now = time::now().to_timespec();
            let t : u64 = (now.sec * 10 + now.nsec as i64 / 100000000) as u64;
            let sig : H520 = sign(&privkey, &H256::from(t)).unwrap().into();
            let hash = sig.sha3();
            if hash < difficulty {
                let blk = Block {
                        height: h+1,
                        timestamp: t,
                        pubkey: H256::zero(),
                        signature: sig.into(),
                        transactions: Vec::new(),
                        pre_hash: pre_hash,
                };
                tx.send(blk).unwrap();
            }

            thread::sleep(Duration::from_millis(100));
        }
    });
}

