extern crate env_logger;
extern crate network;
#[macro_use]
extern crate log;
extern crate clap;
extern crate time;
extern crate bincode;
extern crate util;
extern crate crypto;
extern crate chain;
extern crate miner;
extern crate parking_lot;
extern crate core;
extern crate timesync;

use env_logger::LogBuilder;
use std::env;
use log::{LogLevelFilter, LogRecord};
use util::config::SleepyConfig;
use network::server::start_server;
use network::connection::{start_client, Operation};
use network::msgclass::MsgClass;
use std::sync::mpsc::channel;
use clap::App;
use std::time::Duration;
use std::thread;
use bincode::{serialize, deserialize, Infinite};
use miner::start_miner;
use chain::{Chain, Error};
use std::sync::Arc;
use parking_lot::RwLock;
use core::sleepy::Sleepy;
use timesync::{TimeSyncer, TimeSync};
use std::sync::mpsc::Sender;

pub fn log_init() {
    let format = |record: &LogRecord| {
        let t = time::now();
        format!("{},{:03} - {} - {}",
                time::strftime("%Y-%m-%d %H:%M:%S", &t).unwrap(),
                t.tm_nsec / 1000_000,
                record.level(),
                record.args())
    };

    let mut builder = LogBuilder::new();
    builder.format(format).filter(None, LogLevelFilter::Info);

    if env::var("RUST_LOG").is_ok() {
        builder.parse(&env::var("RUST_LOG").unwrap());
    }

    builder.init().unwrap();
}

pub fn start_time_sync(tx: Sender<(u32, Operation, Vec<u8>)>, syncer : Arc<RwLock<TimeSyncer>>) {
    thread::spawn(move || {
        let tx = tx.clone();
        let syncer = syncer.clone();
        let (id, duration, nodes_size) = {
            let guard = syncer.read();
            (guard.id, guard.duration, guard.size)
        };
        thread::sleep(Duration::from_millis(1000 * id as u64 * duration));
        info!("start time sync!");
        
        loop {
            { syncer.write().next_round(); }
            let mut msg : TimeSync = Default::default();
            {
                let guard = syncer.read();
                msg.t1 = guard.round_time;
                msg.round = guard.round;
            };
            let msg = MsgClass::TIMESYNC(msg);
            let message = serialize(&msg, Infinite).unwrap();
            tx.send((999, Operation::BROADCAST, message)).unwrap();
            thread::sleep(Duration::from_millis(1000 * duration * nodes_size));
        }
    });
}

fn main() {
    env::set_var("RUST_BACKTRACE", "full");

    log_init();

    info!("Sleepy node start...");
    // init app
    let matches = App::new("Sleepy")
        .version("0.1")
        .author("Cryptape")
        .about("Sleepy Node powered by Rust")
        .args_from_usage("-c, --config=[FILE] 'Sets a custom config file'")
        .get_matches();

    let mut config_path = "config";

    if let Some(c) = matches.value_of("config") {
        info!("Value for config: {}", c);
        config_path = c;
    }

    let config = SleepyConfig::new(config_path);

    let my_id = config.getid();
    let nodes_size = config.max_peer + 1;
    let duration = config.duration;

    let (stx, srx) = channel();

    // start server
    // This brings up our server.
    start_server(&config, stx);

    // connect peers
    let (ctx, crx) = channel();
    start_client(&config, crx);


    // start time sync
    let time_syncer = TimeSyncer::new(my_id, nodes_size, 1000, 1.0, duration);
    let time_syncer = Arc::new(RwLock::new(time_syncer));
    start_time_sync(ctx.clone(), time_syncer.clone());

    let config = Arc::new(RwLock::new(config));

    // init chain
    let chain = Chain::init(config.clone(), time_syncer.clone());

    // start miner
    start_miner(ctx.clone(), chain.clone(), config.clone(), time_syncer.clone());

    let sleepy = Sleepy::new(config, time_syncer.clone());

    loop {
        let (origin, msg) = srx.recv().unwrap();
        trace!("get msg from {}", origin);
        let decoded: MsgClass = deserialize(&msg[..]).unwrap();
        match decoded {
            MsgClass::BLOCK(blk) => {
                trace!("get block {} from {}", blk.height, origin);
                let ret = sleepy.verify_block_basic(&blk);
                if ret.is_ok() {
                    let ret = chain.insert(&blk);
                    match ret {
                        Ok(_) => {
                            let message = serialize(&MsgClass::BLOCK(blk), Infinite).unwrap();
                            ctx.send((origin, Operation::SUBTRACT, message)).unwrap();
                        }
                        Err(err) => {
                            if err != Error::Duplicate {
                                warn!("insert block error {:?}", err);
                            }
                            if err == Error::MissParent {
                                let message = serialize(&MsgClass::SYNCREQ(blk.pre_hash), Infinite)
                                    .unwrap();
                                ctx.send((origin, Operation::SINGLE, message)).unwrap();
                            }
                        }
                    }
                } else {
                    warn!("verify block error {:?}", ret);
                }
            }
            MsgClass::SYNCREQ(hash) => {
                info!("request block which hash is {:?}", hash);
                match chain.get_block_by_hash(&hash) {
                    Some(blk) => {
                        let message = serialize(&MsgClass::BLOCK(blk), Infinite).unwrap();
                        ctx.send((origin, Operation::SINGLE, message)).unwrap();
                    }
                    _ => {
                        warn!("not found block by hash");
                    }
                }

            }
            MsgClass::TIMESYNC(mut sync) => {
                if sync.t2 == 0 {
                    // sync reqest
                    sync.t2 = {time_syncer.read().time_now_ms()};
                    info!("time sync request msg {:?}", msg);
                    let message = serialize(&MsgClass::TIMESYNC(sync), Infinite).unwrap();
                    ctx.send((origin, Operation::SINGLE, message)).unwrap();
                } else {
                    // sync response
                     { let _ = time_syncer.write().add_message(sync); }
                }
            }
            MsgClass::MSG(m) => {
                trace!("get msg {:?}", m);
            }
        }
    }
}