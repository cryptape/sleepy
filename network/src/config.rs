extern crate toml;

use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;

#[derive(Debug, RustcDecodable)]
pub struct NetConfig {
    pub id_card:Option<u32>,
    pub port: Option<u64>,
    pub max_peer: Option<u64>,
    pub peers: Option<Vec<PeerConfig>>,
}

#[derive(Debug, RustcDecodable)]
pub struct PeerConfig {
    pub id_card:Option<u32>,
    pub ip: Option<String>,
    pub port: Option<u64>,
}

impl NetConfig {
    pub fn new(path: &str) -> Self {
        let config_file = File::open(path).unwrap();
        let mut fconfig = BufReader::new(config_file);
        let mut content = String::new();
        fconfig.read_to_string(&mut content).unwrap();
        toml::decode_str(&content).unwrap()
    }
}

