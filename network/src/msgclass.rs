use chain::block::Block;
use chain::transaction::SignedTransaction;
use util::hash::H256;

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgClass {
    BLOCK(Block),
    SYNCREQ(H256),
    TX(SignedTransaction),
    MSG(Vec<u8>),
}