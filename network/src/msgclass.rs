use chain::SignedBlock;
use bigint::hash::H256;

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgClass {
    BLOCK(SignedBlock),
    SYNCREQ(H256),
    MSG(Vec<u8>),
}