use types::Block;
use bigint::hash::H256;

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgClass {
    BLOCK(Block),
    SYNCREQ(H256),
    MSG(Vec<u8>),
}