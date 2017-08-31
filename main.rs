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
extern crate tx_pool;

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
use chain::chain::Chain;
use chain::error::Error;
use std::sync::Arc;
use parking_lot::RwLock;
use tx_pool::Pool;

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

    let (stx, srx) = channel();

    // start server
    // This brings up our server.
    start_server(&config, stx);

    //wait for server start
    thread::sleep(Duration::new(5, 0));

    // connect peers
    let (ctx, crx) = channel();
    start_client(&config, crx);

    //make sure connect to other peers
    thread::sleep(Duration::new(20, 0));

    let config = Arc::new(RwLock::new(config));

    // init chain
    let chain = Chain::init(config.clone());

    // init tx pool
    let tx_pool = Pool::new(1000, 300);
    let tx_pool = Arc::new(RwLock::new(tx_pool));

    // start miner
    start_miner(ctx.clone(), chain.clone(), config.clone(), tx_pool.clone());

    loop {
        let (origin, msg) = srx.recv().unwrap();
        trace!("get msg from {}", origin);
        let decoded: MsgClass = deserialize(&msg[..]).unwrap();
        match decoded {
            MsgClass::BLOCK(blk) => {
                trace!("get block {} from {}", blk.height, origin);
                let ret = chain.insert(blk.clone());
                match ret {
                    Ok(_) => {}
                    Err(err) => {
                        if err != Error::DuplicateBlock {
                            warn!("insert block error {:?}", err);
                        }
                        if err == Error::UnknownParent {
                            let message = serialize(&MsgClass::SYNCREQ(blk.parent_hash), Infinite)
                                .unwrap();
                            ctx.send((origin, Operation::SINGLE, message)).unwrap();
                        }
                    }
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
            MsgClass::TX(stx) => {
                let ret = chain.tx_basic_check(&stx);
                if ret.is_ok() {
                    let hash = stx.hash();             
                    let ret = { tx_pool.write().enqueue(stx.clone(), hash) };
                    if ret {
                        let message = serialize(&MsgClass::TX(stx), Infinite).unwrap();
                        ctx.send((origin, Operation::BROADCAST, message)).unwrap();
                    }
                } else {
                    warn!("bad stx {:?}", ret);
                }
            }
            MsgClass::MSG(m) => {
                trace!("get msg {:?}", m);
            }
        }
    }
}