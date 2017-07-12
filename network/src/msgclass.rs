use chain::{SignedBlock, SignedTransaction};
use bigint::hash::{H256};
use timesync::TimeSync;

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgClass {
    BLOCK(SignedBlock),
    SYNCREQ(H256),
    TIMESYNC(TimeSync),
    TX(SignedTransaction),
    MSG(Vec<u8>),
}