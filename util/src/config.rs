extern crate toml;

use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;
use bigint::hash::{H256, H512};
use bigint::uint::U256;
use time;

#[derive(Debug, Deserialize)]
pub struct SleepyConfig {
    pub id_card: u32,
    pub port: u64,
    pub max_peer: u64,
    pub duration: u64,
    pub hz: u64,
    pub miner_private_key: H256,
    pub signer_private_key: H256,
    pub peers: Vec<PeerConfig>,
    pub keygroups: Vec<KeyGroup>,
}

#[derive(Debug, Deserialize)]
pub struct PeerConfig {
    pub id_card: u32,
    pub ip: String,
    pub port: u64,
}

#[derive(Debug, Deserialize)]
pub struct KeyGroup {
    pub miner_public_key: H512,
    pub signer_public_key: H512,
}

impl SleepyConfig {
    pub fn new(path: &str) -> Self {
        let config_file = File::open(path).unwrap();
        let mut fconfig = BufReader::new(config_file);
        let mut content = String::new();
        fconfig.read_to_string(&mut content).unwrap();
        toml::from_str(&content).unwrap()
    }

    pub fn timestamp_now(&self) -> u64 {
        let now = time::now().to_timespec();
        (now.sec * self.hz as i64 + now.nsec as i64 / 1000000000 * self.hz as i64) as u64
    }

    pub fn getid(&self) -> u32 {
        self.id_card
    }

    pub fn get_miner_private_key(&self) -> H256 {
        self.miner_private_key
    }

    pub fn get_signer_private_key(&self) -> H256 {
        self.signer_private_key
    }

    pub fn get_difficulty(&self) -> U256 {
        (U256::max_value() / U256::from((self.max_peer + 1) * self.duration * self.hz)).into()
    }

    pub fn set_signer_private_key(&mut self, private_key: H256) {
        self.signer_private_key = private_key;
    }

    pub fn check_keys(&self, minerkey: H512, signerkey: H512) -> bool {
        let keygroups: &[KeyGroup] = self.keygroups.as_ref();
        for keys in keygroups {
            if keys.miner_public_key == minerkey {
                return keys.signer_public_key == signerkey;
            }
        }

        false
    }

    pub fn replace_signerkey(&mut self, oldkey: H512, newkey: H512) {
        let keygroups: &mut [KeyGroup] = self.keygroups.as_mut();
        for keys in keygroups {
            if keys.signer_public_key == oldkey {
                keys.signer_public_key = newkey;
            }
        }
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
            hz = 10
            duration = 6
            miner_private_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            signer_private_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            [[peers]]
            id_card = 1
            ip = "127.0.0.1"
            port = 40001
            [[peers]]
            id_card = 2
            ip = "127.0.0.1"
            port = 40002
            [[keygroups]]
            miner_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            signer_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            [[keygroups]]
            miner_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            signer_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
        "#;

        let value: SleepyConfig = toml::from_str(toml).unwrap();
        println!("{:?}", value);
        assert_eq!(value.port, 40000);
    }
}
