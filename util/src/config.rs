extern crate toml;

use std::io::prelude::*;
use std::fs::File;
use std::io::BufReader;
use {H256, H512, U256};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::thread;
use std::sync::mpsc;
use time;
use ntp;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub id_card: u32,
    pub port: u64,
    pub max_peer: u64,
    pub steps: u64,
    pub nps: u64,
    pub miner_private_key: Vec<u8>,
    pub signer_private_key: H256,
    pub peers: Vec<PeerConfig>,
    pub keygroups: Vec<KeyGroup>,
    pub epoch_len: u64,
    pub start_time: u64,
    pub ntp_servers: Vec<String>,
    pub buffer_size: u64,
}

#[derive(Debug, Deserialize)]
pub struct SleepyConfig {
    pub config: Config,
    pub public_keys: HashMap<H512, (Vec<u8>, Vec<u8>)>,
}

#[derive(Debug, Deserialize)]
pub struct PeerConfig {
    pub id_card: u32,
    pub ip: String,
    pub port: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KeyGroup {
    pub proof_public_key: Vec<u8>,
    pub proof_public_g: Vec<u8>,
    pub signer_public_key: H512,
}

impl Deref for SleepyConfig {
    type Target = Config;

    #[inline]
    fn deref(&self) -> &Config {
        &self.config
    }
}

impl DerefMut for SleepyConfig {
    #[inline]
    fn deref_mut(&mut self) -> &mut Config {
        &mut self.config
    }
}

impl SleepyConfig {
    pub fn new(path: &str) -> Self {
        let config_file = File::open(path).unwrap();
        let mut fconfig = BufReader::new(config_file);
        let mut content = String::new();
        fconfig.read_to_string(&mut content).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        let mut public_keys = HashMap::new();

        for v in config.keygroups.clone() {
            let miner = (v.proof_public_key, v.proof_public_g);
            public_keys.insert(v.signer_public_key, miner);
        }

        SleepyConfig {
            config: config,
            public_keys: public_keys,
        }
    }

    pub fn get_keygroups(&self) -> &Vec<KeyGroup> {
        self.keygroups.as_ref()
    }

    pub fn get_id(&self) -> u32 {
        self.id_card
    }

    pub fn start_time(&self) -> u64 {
        self.start_time
    }

    pub fn get_miner_private_key(&self) -> Vec<u8> {
        self.miner_private_key.clone()
    }

    pub fn get_signer_private_key(&self) -> H256 {
        self.signer_private_key
    }

    pub fn get_difficulty(&self) -> U256 {
        (U256::max_value() / U256::from((self.max_peer + 1) * self.steps * self.nps)).into()
    }

    pub fn get_proof_pub(&self, sign_key: &H512) -> Option<(Vec<u8>, Vec<u8>)> {
        self.public_keys.get(sign_key).map(|v| v.clone())
    }

    // pub fn check_keys(&self, miner_key: &H512, sign_key: &H512) -> bool {
    //     match self.public_keys.get(miner_key) {
    //         Some(k) => k == sign_key,
    //         None => false,
    //     }
    // }

    pub fn sys_now(&self) -> u64 {
        let now = time::now().to_timespec();
        (now.sec * self.nps as i64 + now.nsec as i64 / (1000000000 / self.nps) as i64) as u64
    }

    pub fn ntp_now(&self) -> Option<u64> {
        let now = self.ntp_timestamp();
        if now == 0 {
            return None;
        }
        Some((now / (1000000000 / self.nps) as i64) as u64)
    }

    pub fn ntp_timestamp(&self) ->i64 {
        let (tx, rx) = mpsc::channel();
        let address = self.ntp_servers.clone();
        let len = address.len();
        for addr in address {
            let tx = tx.clone();

            thread::spawn(move || {
                let time = match ntp::request(addr) {
                    Ok(res) => {
                        let t = time::Timespec::from(res.transmit_time);
                        t.sec * 1000000000 + t.nsec as i64
                    },
                    _ => 0,
                };
                let _ = tx.send(time);
            });
        }
        
        let mut r: i64 = 0;
        for _ in 0..len {
            if let Ok(t) = rx.recv() {
                if t != 0 && r == 0 {
                    r = t
                }
            }
        }
        r
    }
}

#[cfg(test)]
mod test {
    use super::*;
    extern crate toml;
    use std::thread;
    use std::time::Duration;
    #[test]
    fn basics() {
        let toml = r#"
            id_card = 0
            port = 40000
            max_peer = 2
            nps = 10
            steps = 6
            epoch_len = 10
            start_time = 1
            miner_private_key = [30, 135, 112, 146, 247, 176, 37, 100, 64, 82, 243, 99, 209, 43, 226, 150, 182, 2, 80, 33]
            signer_private_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            ntp_servers = ["s1a.time.edu.cn:123", "cn.ntp.org.cn:123" ]
            buffer_size = 5
            
            [[peers]]
            id_card = 1
            ip = "127.0.0.1"
            port = 40001
            [[peers]]
            id_card = 2
            ip = "127.0.0.1"
            port = 40002
            [[keygroups]]
            proof_public_key = [5, 187, 13, 170, 167, 224, 60, 147, 202, 19, 224, 0, 123, 201, 193, 8, 80, 105, 212, 162, 5, 103, 50, 145, 212, 129, 226, 7, 133, 209, 205, 106, 25, 243, 195, 27, 250, 97, 33, 164, 1]
            proof_public_g = [26, 143, 4, 165, 28, 50, 23, 127, 123, 48, 213, 125, 157, 223, 45, 63, 193, 95, 249, 215, 27, 71, 102, 178, 229, 66, 7, 46, 227, 238, 184, 125, 152, 61, 121, 252, 4, 156, 131, 163, 0]
            signer_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae"
            [[keygroups]]
            proof_public_key = [5, 187, 13, 170, 167, 224, 60, 147, 202, 19, 224, 0, 123, 201, 193, 8, 80, 105, 212, 162, 5, 103, 50, 145, 212, 129, 226, 7, 133, 209, 205, 106, 25, 243, 195, 27, 250, 97, 33, 164, 1]
            proof_public_g = [26, 143, 4, 165, 28, 50, 23, 127, 123, 48, 213, 125, 157, 223, 45, 63, 193, 95, 249, 215, 27, 71, 102, 178, 229, 66, 7, 46, 227, 238, 184, 125, 152, 61, 121, 252, 4, 156, 131, 163, 0]
            signer_public_key = "5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5ae5a39ed1020c04d4d84539975b893a4e7c53eab6c2965db8bc3468093a31bc5af"
        "#;

        let value: Config = toml::from_str(toml).unwrap();
        let config = SleepyConfig {config: value, public_keys: HashMap::new()};
        println!("{:?}", config);
        assert_eq!(config.port, 40000);

        let _ = config.ntp_now();
        thread::sleep(Duration::from_millis(100));
        let _ = config.ntp_now();
        // assert_eq!(t1 - t, 1);
    }
}
