extern crate env_logger;
extern crate network;
#[macro_use]
extern crate log;
extern crate clap;
extern crate time;

use env_logger::LogBuilder;
use std::env;
use log::{LogLevelFilter, LogRecord};
use network::config::NetConfig;
use network::server::{MySender, start_server};
use network::connection::{Connection, do_connect, broadcast, Operation};
use std::sync::mpsc::channel;
use clap::App;
use std::time::Duration;
use std::thread;

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

    let config = NetConfig::new(config_path);

    let (tx, rx) = channel();

    // start server
    // This brings up our server.
    let mysender = MySender::new(tx);
    start_server(&config, mysender);

    // connect peers
    let con = Connection::new(&config);
    do_connect(&con);

    thread::sleep(Duration::from_millis(3000));
    broadcast(&con, [1,2,3,4].to_vec(), 0, Operation::BROADCAST);
    loop {
        let (origin, msg) = rx.recv().unwrap();
        info!("get msg {:?} from {}", msg, origin);
        thread::sleep(Duration::from_millis(1000));
        broadcast(&con, [1,2,3,4].to_vec(), origin, Operation::BROADCAST);
    }
}