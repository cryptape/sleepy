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

use env_logger::LogBuilder;
use std::env;
use log::{LogLevelFilter, LogRecord};
use network::config::SleepyConfig;
use network::server::start_server;
use network::connection::{start_client, Operation};
use std::sync::mpsc::channel;
use clap::App;
use std::time::Duration;
use std::thread;
use bincode::{serialize, deserialize, Infinite};
use miner::start_miner;
use chain::{Block, Chain};
use std::sync::Arc;
use parking_lot::RwLock;

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
        .author("IC3")
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

    // connect peers
    let (ctx, crx) = channel();
    start_client(&config, crx);

    // init chain
    let chain = Chain::init();
    let chain = Arc::new(chain);

    // start miner
    let config = Arc::new(RwLock::new(config));
    start_miner(ctx.clone(), chain, config);

    loop {
        let (origin, msg) = srx.recv().unwrap();
        info!("get msg {:?} from {}", msg, origin);
        thread::sleep(Duration::from_millis(1000));
        ctx.send((origin, Operation::BROADCAST, [1, 2, 3, 4].to_vec()))
            .unwrap();
    }
}