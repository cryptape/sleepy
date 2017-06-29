extern crate toml;

use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;
use util::hash::H256; 

#[derive(Debug, Deserialize)]
pub struct SleepyConfig {
    pub id_card:Option<u32>,
    pub port: Option<u64>,
    pub max_peer: Option<u64>,
    pub private_key: Option<H256>,
    pub peers: Option<Vec<PeerConfig>>,
}

#[derive(Debug, Deserialize)]
pub struct PeerConfig {
    pub id_card:Option<u32>,
    pub ip: Option<String>,
    pub port: Option<u64>,
    pub public_key: Option<H256>,
}

impl SleepyConfig {
    pub fn new(path: &str) -> Self {
        let config_file = File::open(path).unwrap();
        let mut fconfig = BufReader::new(config_file);
        let mut content = String::new();
        fconfig.read_to_string(&mut content).unwrap();
        toml::from_str(&content).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::SleepyConfig;
    extern crate toml;
    #[test]
    fn basics() {
        let toml = r#"
            id_card = 0
            port = 40000
            max_peer = 2
            private_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            [[peers]]
            id_card = 1
            ip = "127.0.0.1"
            port = 40001
            public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            [[peers]]
            id_card = 2
            ip = "127.0.0.1"
            port = 40002
            public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
        "#;

        let value: SleepyConfig = toml::from_str(toml).unwrap();
        println!("{:?}", value);
        assert_eq!(value.port, Some(40000));
    }
}


