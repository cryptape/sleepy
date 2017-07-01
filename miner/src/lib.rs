extern crate util;
extern crate time;
extern crate crypto;
extern crate bigint;
#[macro_use]
extern crate log;
extern crate network;
extern crate bincode;
extern crate parking_lot;

use std::sync::mpsc::Sender;
use util::types::{Block, Chain};
use std::thread;
use std::time::Duration;
use crypto::{PrivKey, sign};
use bigint::hash::{H256, H520};
use bigint::uint::U256;
use util::Hashable;
use std::sync::Arc;
use network::connection::Operation;
use network::config::SleepyConfig;
use bincode::{serialize, deserialize, Infinite};
use parking_lot::RwLock;

pub fn start_miner(tx: Sender<(u32, Operation, Vec<u8>)>, chain: Arc<Chain>, config: Arc<RwLock<SleepyConfig>>) {
    let difficulty : H256 = (U256::max_value() / U256::from(4 * 6 * 10)).into();
    let tx = tx.clone();
    let chain = chain.clone();
    let config = config.clone();
    thread::spawn(move || {
        info!("start mining!");
        loop {
            let now = time::now().to_timespec();
            let t : u64 = (now.sec * 10 + now.nsec as i64 / 100000000) as u64;
            let mut miner_privkey;
            {
                miner_privkey = config.read().get_miner_private_key();
            }
            let sig : H520 = sign(&miner_privkey, &H256::from(t)).unwrap().into();
            let hash = sig.sha3();
            if hash < difficulty {
                loop {
                    let (h, pre_hash) = chain.get_status();
                    let blk = Block {
                            height: h+1,
                            timestamp: t,
                            pubkey: H256::zero(),
                            signature: sig.into(),
                            transactions: Vec::new(),
                            pre_hash: pre_hash,
                    };
                    let message = serialize(&blk, Infinite).unwrap();
                    if chain.insert(blk).is_ok() {
                        info!("get a block {} {}", h+1, t);
                        let mut signer_privkey;
                        let mut id;
                        {
                            signer_privkey = config.read().get_signer_private_key();
                            id = config.read().getid();
                        }
                        let sig : H520 = sign(&signer_privkey, &message.sha3()).unwrap().into();
                        let msg = serialize(&(message, sig), Infinite).unwrap();
                        tx.send((id, Operation::BROADCAST, msg)).unwrap();
                        break;
                    }
                }
            }

            thread::sleep(Duration::from_millis(100));
        }
    });
}

